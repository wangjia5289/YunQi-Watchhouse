use std::collections::{BTreeMap, HashMap};

use chrono::{Local, LocalResult, NaiveDate, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};

use crate::{
    activity::ActivityState,
    database::{ActivityRecord, ActivityRepository},
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeRange {
    pub start_ms: i64,
    pub end_ms: i64,
}

impl TimeRange {
    pub fn new(start_ms: i64, end_ms: i64) -> AppResult<Self> {
        if end_ms <= start_ms {
            return Err(AppError::InvalidTimeRange(
                "end must be after start".to_owned(),
            ));
        }
        Ok(Self { start_ms, end_ms })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodaySummary {
    pub date: String,
    pub range: TimeRange,
    pub active_duration_ms: i64,
    pub idle_duration_ms: i64,
    pub first_activity_at_ms: Option<i64>,
    pub last_activity_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineEntry {
    pub session_id: i64,
    pub application_id: Option<i64>,
    pub state: ActivityState,
    pub application_name: Option<String>,
    pub bundle_identifier: Option<String>,
    pub category: Option<String>,
    pub window_title: Option<String>,
    pub started_at_ms: i64,
    pub ended_at_ms: i64,
    pub duration_ms: i64,
    pub is_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUsage {
    pub application_id: i64,
    pub application_name: String,
    pub bundle_identifier: Option<String>,
    pub category: String,
    pub is_ignored: bool,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub date: String,
    pub active_duration_ms: i64,
    pub idle_duration_ms: i64,
}

#[derive(Clone)]
pub struct StatisticsService {
    repository: ActivityRepository,
}

impl StatisticsService {
    pub fn new(repository: ActivityRepository) -> Self {
        Self { repository }
    }

    pub fn today_summary(&self) -> AppResult<TodaySummary> {
        self.summary_for_date(Local::now().date_naive())
    }

    pub fn summary_for_date(&self, date: NaiveDate) -> AppResult<TodaySummary> {
        let range = local_day_range(date)?;
        let records = self
            .repository
            .records_overlapping(range.start_ms, range.end_ms)?;
        let mut summary = TodaySummary {
            date: date.to_string(),
            range,
            active_duration_ms: 0,
            idle_duration_ms: 0,
            first_activity_at_ms: None,
            last_activity_at_ms: None,
        };

        for record in records {
            let Some((start, end)) = clipped_bounds(&record, range) else {
                continue;
            };
            let duration = end - start;
            match record.session.state {
                ActivityState::Active => {
                    summary.active_duration_ms += duration;
                    summary.first_activity_at_ms = Some(
                        summary
                            .first_activity_at_ms
                            .map_or(start, |current| current.min(start)),
                    );
                    summary.last_activity_at_ms = Some(
                        summary
                            .last_activity_at_ms
                            .map_or(end, |current| current.max(end)),
                    );
                }
                ActivityState::Idle => summary.idle_duration_ms += duration,
            }
        }
        Ok(summary)
    }

    pub fn timeline_for_date(&self, date: NaiveDate) -> AppResult<Vec<TimelineEntry>> {
        let range = local_day_range(date)?;
        self.timeline(range)
    }

    pub fn timeline(&self, range: TimeRange) -> AppResult<Vec<TimelineEntry>> {
        Ok(self
            .repository
            .records_overlapping(range.start_ms, range.end_ms)?
            .into_iter()
            .filter_map(|record| {
                let (start, end) = clipped_bounds(&record, range)?;
                Some(TimelineEntry {
                    session_id: record.session.id,
                    application_id: record
                        .application
                        .as_ref()
                        .map(|application| application.id),
                    state: record.session.state,
                    application_name: record
                        .application
                        .as_ref()
                        .map(|application| application.name.clone()),
                    bundle_identifier: record
                        .application
                        .as_ref()
                        .and_then(|application| application.bundle_id.clone()),
                    category: record
                        .application
                        .as_ref()
                        .map(|application| application.category.clone()),
                    window_title: record.session.window_title,
                    started_at_ms: start,
                    ended_at_ms: end,
                    duration_ms: end - start,
                    is_open: record.session.is_open,
                })
            })
            .collect())
    }

    pub fn app_usage(&self, range: TimeRange) -> AppResult<Vec<AppUsage>> {
        let records = self
            .repository
            .records_overlapping(range.start_ms, range.end_ms)?;
        let mut usage: HashMap<i64, AppUsage> = HashMap::new();

        for record in records {
            if record.session.state != ActivityState::Active {
                continue;
            }
            let Some((start, end)) = clipped_bounds(&record, range) else {
                continue;
            };
            let Some(application) = record.application else {
                continue;
            };
            let item = usage.entry(application.id).or_insert_with(|| AppUsage {
                application_id: application.id,
                application_name: application.name,
                bundle_identifier: application.bundle_id,
                category: application.category,
                is_ignored: application.is_ignored,
                duration_ms: 0,
            });
            item.duration_ms += end - start;
        }

        let mut usage = usage.into_values().collect::<Vec<_>>();
        usage.sort_by(|left, right| {
            right
                .duration_ms
                .cmp(&left.duration_ms)
                .then_with(|| left.application_name.cmp(&right.application_name))
        });
        Ok(usage)
    }

    pub fn daily_usage(&self, range: TimeRange) -> AppResult<Vec<DailyUsage>> {
        let records = self
            .repository
            .records_overlapping(range.start_ms, range.end_ms)?;
        let mut days: BTreeMap<NaiveDate, (i64, i64)> = BTreeMap::new();

        for record in records {
            let Some((mut cursor, end)) = clipped_bounds(&record, range) else {
                continue;
            };
            while cursor < end {
                let local = Local.timestamp_millis_opt(cursor).single().ok_or_else(|| {
                    AppError::InvalidTimeRange("timestamp cannot be represented locally".to_owned())
                })?;
                let date = local.date_naive();
                let next_date = date
                    .succ_opt()
                    .ok_or_else(|| AppError::InvalidTimeRange("local date overflow".to_owned()))?;
                let next_midnight = local_day_range(next_date)?.start_ms;
                let segment_end = end.min(next_midnight);
                let totals = days.entry(date).or_default();
                match record.session.state {
                    ActivityState::Active => totals.0 += segment_end - cursor,
                    ActivityState::Idle => totals.1 += segment_end - cursor,
                }
                cursor = segment_end;
            }
        }

        Ok(days
            .into_iter()
            .map(
                |(date, (active_duration_ms, idle_duration_ms))| DailyUsage {
                    date: date.to_string(),
                    active_duration_ms,
                    idle_duration_ms,
                },
            )
            .collect())
    }

    pub fn application_daily_usage(
        &self,
        application_id: i64,
        range: TimeRange,
    ) -> AppResult<Vec<DailyUsage>> {
        let records = self
            .repository
            .records_overlapping(range.start_ms, range.end_ms)?;
        let mut days: BTreeMap<NaiveDate, i64> = BTreeMap::new();

        for record in records {
            if record.session.state != ActivityState::Active
                || record.session.application_id != Some(application_id)
            {
                continue;
            }
            let Some((mut cursor, end)) = clipped_bounds(&record, range) else {
                continue;
            };
            while cursor < end {
                let local = Local.timestamp_millis_opt(cursor).single().ok_or_else(|| {
                    AppError::InvalidTimeRange("timestamp cannot be represented locally".to_owned())
                })?;
                let date = local.date_naive();
                let next_date = date
                    .succ_opt()
                    .ok_or_else(|| AppError::InvalidTimeRange("local date overflow".to_owned()))?;
                let segment_end = end.min(local_day_range(next_date)?.start_ms);
                *days.entry(date).or_default() += segment_end - cursor;
                cursor = segment_end;
            }
        }

        Ok(days
            .into_iter()
            .map(|(date, active_duration_ms)| DailyUsage {
                date: date.to_string(),
                active_duration_ms,
                idle_duration_ms: 0,
            })
            .collect())
    }
}

fn clipped_bounds(record: &ActivityRecord, range: TimeRange) -> Option<(i64, i64)> {
    let start = record.session.started_at_ms.max(range.start_ms);
    let end = record.session.ended_at_ms.min(range.end_ms);
    (end > start).then_some((start, end))
}

pub fn local_day_range(date: NaiveDate) -> AppResult<TimeRange> {
    let next_date = date
        .succ_opt()
        .ok_or_else(|| AppError::InvalidTimeRange("local date overflow".to_owned()))?;
    TimeRange::new(
        resolve_local_midnight(date)?,
        resolve_local_midnight(next_date)?,
    )
}

fn resolve_local_midnight(date: NaiveDate) -> AppResult<i64> {
    let midnight = date.and_time(NaiveTime::MIN);
    match Local.from_local_datetime(&midnight) {
        LocalResult::Single(value) => Ok(value.timestamp_millis()),
        LocalResult::Ambiguous(earliest, _) => Ok(earliest.timestamp_millis()),
        LocalResult::None => Err(AppError::InvalidTimeRange(format!(
            "local midnight does not exist for {date}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        activity::{ClosedReason, NewApplication, NewSession},
        database::Database,
    };

    use super::*;

    fn setup() -> (StatisticsService, ActivityRepository, i64) {
        let repository =
            ActivityRepository::new(Database::in_memory().expect("database should open"));
        let application = repository
            .upsert_application(&NewApplication {
                name: "IDEA".to_owned(),
                bundle_id: Some("com.jetbrains.intellij".to_owned()),
                executable_path: None,
                seen_at_ms: 0,
            })
            .expect("application should be stored");
        (
            StatisticsService::new(repository.clone()),
            repository,
            application.id,
        )
    }

    fn store_session(
        repository: &ActivityRepository,
        state: ActivityState,
        application_id: Option<i64>,
        start_ms: i64,
        end_ms: i64,
    ) {
        let session = repository
            .create_session(&NewSession {
                state,
                application_id,
                window_title: None,
                started_at_ms: start_ms,
            })
            .expect("session should open");
        repository
            .close_session(session.id, end_ms, ClosedReason::AppChanged)
            .expect("session should close");
    }

    #[test]
    fn timeline_clips_session_to_requested_range() {
        let (service, repository, application_id) = setup();
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            50,
            250,
        );

        let timeline = service
            .timeline(TimeRange::new(100, 200).expect("range should be valid"))
            .expect("timeline should succeed");
        assert_eq!(timeline.len(), 1);
        assert_eq!(
            (
                timeline[0].started_at_ms,
                timeline[0].ended_at_ms,
                timeline[0].duration_ms,
            ),
            (100, 200, 100)
        );
    }

    #[test]
    fn app_usage_aggregates_active_sessions_and_excludes_idle() {
        let (service, repository, application_id) = setup();
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            100,
            200,
        );
        store_session(&repository, ActivityState::Idle, None, 200, 300);
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            300,
            450,
        );

        let usage = service
            .app_usage(TimeRange::new(100, 450).expect("range should be valid"))
            .expect("usage should succeed");
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].duration_ms, 250);
    }

    #[test]
    fn application_daily_usage_filters_and_aggregates_the_selected_application() {
        let (service, repository, application_id) = setup();
        let other = repository
            .upsert_application(&NewApplication {
                name: "Browser".to_owned(),
                bundle_id: Some("example.browser".to_owned()),
                executable_path: None,
                seen_at_ms: 0,
            })
            .expect("application should be stored");
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            100,
            250,
        );
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            300,
            400,
        );
        store_session(&repository, ActivityState::Active, Some(other.id), 450, 900);

        let daily = service
            .application_daily_usage(
                application_id,
                TimeRange::new(0, 1_000).expect("range should be valid"),
            )
            .expect("daily usage should succeed");
        assert_eq!(daily.len(), 1);
        assert_eq!(daily[0].active_duration_ms, 250);
        assert_eq!(daily[0].idle_duration_ms, 0);
    }

    #[test]
    fn daily_usage_splits_legacy_cross_midnight_session() {
        let (service, repository, application_id) = setup();
        let first_date = Local::now().date_naive();
        let second_date = first_date.succ_opt().expect("next date should exist");
        let first_range = local_day_range(first_date).expect("first range should exist");
        let second_range = local_day_range(second_date).expect("second range should exist");
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            first_range.end_ms - 1_000,
            second_range.start_ms + 1_000,
        );

        let days = service
            .daily_usage(
                TimeRange::new(first_range.end_ms - 2_000, second_range.start_ms + 2_000)
                    .expect("range should be valid"),
            )
            .expect("daily usage should succeed");
        assert_eq!(days.len(), 2);
        assert_eq!(days[0].active_duration_ms, 1_000);
        assert_eq!(days[1].active_duration_ms, 1_000);
    }
}
