use std::collections::{BTreeMap, HashMap, HashSet};

use chrono::{
    Datelike, Duration, Local, LocalResult, NaiveDate, NaiveTime, TimeZone, Timelike, Weekday,
};
use serde::{Deserialize, Serialize};

use crate::{
    activity::ActivityState,
    database::{
        ActivityRecord, ActivityRepository, ActivityTag, Project, SessionOrganization,
        TimelineSearch, UsageLimitRule, UsageLimitScopeType,
    },
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
    pub note: Option<String>,
    pub project: Option<Project>,
    pub tags: Vec<ActivityTag>,
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
    pub record_window_titles: bool,
    pub duration_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryUsage {
    pub category: String,
    pub duration_ms: i64,
    pub application_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationUsage {
    pub id: i64,
    pub name: String,
    pub color: String,
    pub duration_ms: i64,
    pub session_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationInsights {
    pub project_usage: Vec<OrganizationUsage>,
    pub tag_usage: Vec<OrganizationUsage>,
    pub unassigned_active_duration_ms: i64,
    pub unassigned_session_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusBlock {
    pub started_at_ms: i64,
    pub ended_at_ms: i64,
    pub active_duration_ms: i64,
    pub application_switch_count: i64,
    pub is_open: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSummary {
    pub total_focus_duration_ms: i64,
    pub longest_focus_duration_ms: i64,
    pub application_switch_count: i64,
    pub goal_minutes: i64,
    pub break_reminders_enabled: bool,
    pub break_reminder_minutes: i64,
    pub quiet_hours_start: String,
    pub quiet_hours_end: String,
    pub blocks: Vec<FocusBlock>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyUsage {
    pub date: String,
    pub active_duration_ms: i64,
    pub idle_duration_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HourlyUsage {
    pub hour: u32,
    pub active_duration_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductivityReport {
    pub range: TimeRange,
    pub active_duration_ms: i64,
    pub idle_duration_ms: i64,
    pub previous_active_duration_ms: i64,
    pub previous_idle_duration_ms: i64,
    pub daily_usage: Vec<DailyUsage>,
    pub hourly_usage: Vec<HourlyUsage>,
    pub category_usage: Vec<CategoryUsage>,
    pub organization_insights: OrganizationInsights,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelinePage {
    pub entries: Vec<TimelineEntry>,
    pub total_count: usize,
    pub active_duration_ms: i64,
    pub idle_duration_ms: i64,
    pub offset: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UsageLimitThresholdState {
    Below80,
    Reached80,
    Reached100,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageLimitProgress {
    pub id: i64,
    pub scope_type: UsageLimitScopeType,
    pub application_id: Option<i64>,
    pub application_name: Option<String>,
    pub category: Option<String>,
    pub weekday_limit_minutes: i64,
    pub weekend_limit_minutes: i64,
    pub notifications_enabled: bool,
    pub enabled: bool,
    pub local_date: String,
    pub base_limit_minutes: i64,
    pub limit_minutes: i64,
    pub temporary_added_minutes: i64,
    pub notifications_snoozed_until_ms: Option<i64>,
    pub notifications_silenced: bool,
    pub used_duration_ms: i64,
    pub percentage: f64,
    pub threshold_state: UsageLimitThresholdState,
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

    pub fn today_usage_limit_progress(&self) -> AppResult<Vec<UsageLimitProgress>> {
        self.usage_limit_progress_for_date(Local::now().date_naive())
    }

    pub fn today_notifiable_usage_limit_progress(&self) -> AppResult<Vec<UsageLimitProgress>> {
        let now = Local::now();
        let date = now.date_naive();
        let now_ms = now.timestamp_millis();
        let rules = self
            .repository
            .usage_limit_rules()?
            .into_iter()
            .filter(|rule| rule.enabled && rule.notifications_enabled);
        Ok(self
            .usage_limit_progress_for_rules(date, rules)?
            .into_iter()
            .filter(|progress| {
                !progress.notifications_silenced
                    && progress
                        .notifications_snoozed_until_ms
                        .is_none_or(|until_ms| until_ms <= now_ms)
            })
            .collect())
    }

    pub fn usage_limit_progress_for_date(
        &self,
        date: NaiveDate,
    ) -> AppResult<Vec<UsageLimitProgress>> {
        self.usage_limit_progress_for_rules(date, self.repository.usage_limit_rules()?)
    }

    fn usage_limit_progress_for_rules(
        &self,
        date: NaiveDate,
        rules: impl IntoIterator<Item = UsageLimitRule>,
    ) -> AppResult<Vec<UsageLimitProgress>> {
        let range = local_day_range(date)?;
        rules
            .into_iter()
            .map(|rule| {
                let used_duration_ms = self.repository.active_usage_duration_for_rule(
                    &rule,
                    range.start_ms,
                    range.end_ms,
                )?;
                let exception = self
                    .repository
                    .usage_limit_daily_exception(rule.id, &date.to_string())?;
                Ok(usage_limit_progress(
                    rule,
                    date,
                    used_duration_ms,
                    exception,
                ))
            })
            .collect()
    }

    pub fn today_focus_summary(&self) -> AppResult<FocusSummary> {
        let settings = self.repository.settings()?;
        let mut summary = self.focus_summary(
            local_day_range(Local::now().date_naive())?,
            settings.focus_block_gap_minutes,
            settings.daily_focus_goal_minutes,
        )?;
        summary.break_reminders_enabled = settings.break_reminders_enabled;
        summary.break_reminder_minutes = settings.break_reminder_minutes;
        summary.quiet_hours_start = settings.quiet_hours_start;
        summary.quiet_hours_end = settings.quiet_hours_end;
        Ok(summary)
    }

    pub fn focus_summary(
        &self,
        range: TimeRange,
        gap_minutes: i64,
        goal_minutes: i64,
    ) -> AppResult<FocusSummary> {
        let records = self
            .repository
            .records_overlapping(range.start_ms, range.end_ms)?;
        let gap_ms = gap_minutes.saturating_mul(60_000);
        let mut blocks = Vec::new();
        let mut current: Option<FocusBlockBuilder> = None;

        for record in records {
            let Some((start, end)) = clipped_bounds(&record, range) else {
                continue;
            };
            if record.session.state == ActivityState::Idle {
                if end - start >= gap_ms
                    && let Some(block) = current.take()
                {
                    blocks.push(block.finish());
                }
                continue;
            }
            let application_id = record
                .application
                .as_ref()
                .map(|application| application.id);
            if current
                .as_ref()
                .is_some_and(|block| start.saturating_sub(block.ended_at_ms) >= gap_ms)
                && let Some(block) = current.take()
            {
                blocks.push(block.finish());
            }
            let block = current.get_or_insert(FocusBlockBuilder {
                started_at_ms: start,
                ended_at_ms: end,
                active_duration_ms: 0,
                application_switch_count: 0,
                last_application_id: application_id,
                is_open: false,
            });
            if block.last_application_id.is_some()
                && application_id.is_some()
                && block.last_application_id != application_id
            {
                block.application_switch_count += 1;
            }
            block.last_application_id = application_id;
            block.ended_at_ms = block.ended_at_ms.max(end);
            block.active_duration_ms += end - start;
            block.is_open = record.session.is_open;
        }
        if let Some(block) = current {
            blocks.push(block.finish());
        }

        Ok(FocusSummary {
            total_focus_duration_ms: blocks.iter().map(|block| block.active_duration_ms).sum(),
            longest_focus_duration_ms: blocks
                .iter()
                .map(|block| block.active_duration_ms)
                .max()
                .unwrap_or_default(),
            application_switch_count: blocks
                .iter()
                .map(|block| block.application_switch_count)
                .sum(),
            goal_minutes,
            break_reminders_enabled: false,
            break_reminder_minutes: 60,
            quiet_hours_start: "22:00".to_owned(),
            quiet_hours_end: "08:00".to_owned(),
            blocks,
        })
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

    pub fn timeline_page_for_date(
        &self,
        date: NaiveDate,
        offset: usize,
        limit: usize,
    ) -> AppResult<TimelinePage> {
        self.timeline_page_for_date_filtered(date, offset, limit, &TimelineSearch::default())
    }

    pub fn timeline_page_for_date_filtered(
        &self,
        date: NaiveDate,
        offset: usize,
        limit: usize,
        search: &TimelineSearch,
    ) -> AppResult<TimelinePage> {
        let range = local_day_range(date)?;
        self.timeline_page_filtered(range, offset, limit, search, false)
    }

    pub fn timeline_page_for_date_range_filtered(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
        offset: usize,
        limit: usize,
        search: &TimelineSearch,
    ) -> AppResult<TimelinePage> {
        let range = local_date_range_inclusive(start_date, end_date)?;
        self.timeline_page_filtered(range, offset, limit, search, true)
    }

    fn timeline_page_filtered(
        &self,
        range: TimeRange,
        offset: usize,
        limit: usize,
        search: &TimelineSearch,
        newest_first: bool,
    ) -> AppResult<TimelinePage> {
        let (total_count, active_duration_ms, idle_duration_ms) = self
            .repository
            .timeline_page_totals_filtered(range.start_ms, range.end_ms, search)?;
        let records = if newest_first {
            self.repository
                .records_overlapping_page_filtered_descending(
                    range.start_ms,
                    range.end_ms,
                    offset,
                    limit,
                    search,
                )?
        } else {
            self.repository.records_overlapping_page_filtered(
                range.start_ms,
                range.end_ms,
                offset,
                limit,
                search,
            )?
        };
        let organizations = self.repository.session_organizations(
            &records
                .iter()
                .map(|record| record.session.id)
                .collect::<Vec<_>>(),
        )?;
        let entries = timeline_entries(records, range, organizations);
        Ok(TimelinePage {
            has_more: offset.saturating_add(entries.len()) < total_count,
            entries,
            total_count,
            active_duration_ms,
            idle_duration_ms,
            offset,
        })
    }

    pub fn timeline(&self, range: TimeRange) -> AppResult<Vec<TimelineEntry>> {
        let records = self
            .repository
            .records_overlapping(range.start_ms, range.end_ms)?;
        let organizations = self.repository.session_organizations(
            &records
                .iter()
                .map(|record| record.session.id)
                .collect::<Vec<_>>(),
        )?;
        Ok(timeline_entries(records, range, organizations))
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
                record_window_titles: application.record_window_titles,
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

    pub fn category_usage(&self, range: TimeRange) -> AppResult<Vec<CategoryUsage>> {
        let records = self
            .repository
            .records_overlapping(range.start_ms, range.end_ms)?;
        Ok(category_usage_from_records(&records, range))
    }

    pub fn daily_usage(&self, range: TimeRange) -> AppResult<Vec<DailyUsage>> {
        let records = self
            .repository
            .records_overlapping(range.start_ms, range.end_ms)?;
        daily_usage_from_records(&records, range)
    }

    pub fn productivity_report(&self, range: TimeRange) -> AppResult<ProductivityReport> {
        let duration = range.end_ms - range.start_ms;
        let previous = TimeRange::new(range.start_ms.saturating_sub(duration), range.start_ms)?;
        let records = self
            .repository
            .records_overlapping(previous.start_ms, range.end_ms)?;
        let (previous_active_duration_ms, previous_idle_duration_ms) =
            duration_totals_from_records(&records, previous);
        let records = records
            .into_iter()
            .filter(|record| clipped_bounds(record, range).is_some())
            .collect::<Vec<_>>();
        let daily_usage = daily_usage_from_records(&records, range)?;
        let category_usage = category_usage_from_records(&records, range);
        let organizations = self.repository.session_organizations(
            &records
                .iter()
                .map(|record| record.session.id)
                .collect::<Vec<_>>(),
        )?;
        let timeline = timeline_entries(records, range, organizations);
        let mut hourly = [0_i64; 24];
        let mut active_duration_ms = 0;
        let mut idle_duration_ms = 0;

        for entry in &timeline {
            match entry.state {
                ActivityState::Active => {
                    active_duration_ms += entry.duration_ms;
                    let mut cursor = entry.started_at_ms;
                    while cursor < entry.ended_at_ms {
                        let local =
                            Local.timestamp_millis_opt(cursor).single().ok_or_else(|| {
                                AppError::InvalidTimeRange(
                                    "timestamp cannot be represented locally".to_owned(),
                                )
                            })?;
                        let hour = local.hour() as usize;
                        let next_hour = local
                            .with_minute(0)
                            .and_then(|value| value.with_second(0))
                            .and_then(|value| value.with_nanosecond(0))
                            .map(|value| value.timestamp_millis().saturating_add(3_600_000))
                            .unwrap_or(entry.ended_at_ms);
                        let segment_end = entry.ended_at_ms.min(next_hour);
                        hourly[hour] += segment_end.saturating_sub(cursor);
                        cursor = segment_end;
                    }
                }
                ActivityState::Idle => idle_duration_ms += entry.duration_ms,
            }
        }
        Ok(ProductivityReport {
            range,
            active_duration_ms,
            idle_duration_ms,
            previous_active_duration_ms,
            previous_idle_duration_ms,
            daily_usage,
            hourly_usage: hourly
                .into_iter()
                .enumerate()
                .map(|(hour, active_duration_ms)| HourlyUsage {
                    hour: hour as u32,
                    active_duration_ms,
                })
                .collect(),
            category_usage,
            organization_insights: organization_insights(&timeline),
        })
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

fn duration_totals_from_records(records: &[ActivityRecord], range: TimeRange) -> (i64, i64) {
    records.iter().fold((0, 0), |mut totals, record| {
        let Some((start, end)) = clipped_bounds(record, range) else {
            return totals;
        };
        match record.session.state {
            ActivityState::Active => totals.0 += end - start,
            ActivityState::Idle => totals.1 += end - start,
        }
        totals
    })
}

fn category_usage_from_records(records: &[ActivityRecord], range: TimeRange) -> Vec<CategoryUsage> {
    let mut usage: HashMap<String, (i64, HashSet<i64>)> = HashMap::new();
    for record in records {
        if record.session.state != ActivityState::Active {
            continue;
        }
        let Some((start, end)) = clipped_bounds(record, range) else {
            continue;
        };
        let Some(application) = record.application.as_ref() else {
            continue;
        };
        let category = record
            .effective_category
            .as_ref()
            .unwrap_or(&application.category)
            .clone();
        let item = usage.entry(category).or_default();
        item.0 += end - start;
        item.1.insert(application.id);
    }

    let mut usage = usage
        .into_iter()
        .map(|(category, (duration_ms, applications))| CategoryUsage {
            category,
            duration_ms,
            application_count: applications.len(),
        })
        .collect::<Vec<_>>();
    usage.sort_by(|left, right| {
        right
            .duration_ms
            .cmp(&left.duration_ms)
            .then_with(|| left.category.cmp(&right.category))
    });
    usage
}

fn daily_usage_from_records(
    records: &[ActivityRecord],
    range: TimeRange,
) -> AppResult<Vec<DailyUsage>> {
    let mut days: BTreeMap<NaiveDate, (i64, i64)> = BTreeMap::new();
    for record in records {
        let Some((mut cursor, end)) = clipped_bounds(record, range) else {
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

struct FocusBlockBuilder {
    started_at_ms: i64,
    ended_at_ms: i64,
    active_duration_ms: i64,
    application_switch_count: i64,
    last_application_id: Option<i64>,
    is_open: bool,
}

impl FocusBlockBuilder {
    fn finish(self) -> FocusBlock {
        FocusBlock {
            started_at_ms: self.started_at_ms,
            ended_at_ms: self.ended_at_ms,
            active_duration_ms: self.active_duration_ms,
            application_switch_count: self.application_switch_count,
            is_open: self.is_open,
        }
    }
}

fn usage_limit_progress(
    rule: UsageLimitRule,
    date: NaiveDate,
    used_duration_ms: i64,
    exception: crate::database::UsageLimitDailyException,
) -> UsageLimitProgress {
    let base_limit_minutes = if matches!(date.weekday(), Weekday::Sat | Weekday::Sun) {
        rule.weekend_limit_minutes
    } else {
        rule.weekday_limit_minutes
    };
    let limit_minutes = base_limit_minutes.saturating_add(exception.temporary_added_minutes);
    let limit_ms = limit_minutes.saturating_mul(60_000);
    let used_duration_ms = used_duration_ms.max(0);
    let threshold_state = if used_duration_ms.saturating_mul(100) >= limit_ms.saturating_mul(100) {
        UsageLimitThresholdState::Reached100
    } else if used_duration_ms.saturating_mul(100) >= limit_ms.saturating_mul(80) {
        UsageLimitThresholdState::Reached80
    } else {
        UsageLimitThresholdState::Below80
    };
    UsageLimitProgress {
        id: rule.id,
        scope_type: rule.scope_type,
        application_id: rule.application_id,
        application_name: rule.application_name,
        category: rule.category,
        weekday_limit_minutes: rule.weekday_limit_minutes,
        weekend_limit_minutes: rule.weekend_limit_minutes,
        notifications_enabled: rule.notifications_enabled,
        enabled: rule.enabled,
        local_date: date.to_string(),
        base_limit_minutes,
        limit_minutes,
        temporary_added_minutes: exception.temporary_added_minutes,
        notifications_snoozed_until_ms: exception.notifications_snoozed_until_ms,
        notifications_silenced: exception.notifications_silenced,
        used_duration_ms,
        percentage: used_duration_ms as f64 / limit_ms as f64 * 100.0,
        threshold_state,
    }
}

fn timeline_entries(
    records: Vec<ActivityRecord>,
    range: TimeRange,
    mut organizations: HashMap<i64, SessionOrganization>,
) -> Vec<TimelineEntry> {
    records
        .into_iter()
        .filter_map(|record| {
            let (start, end) = clipped_bounds(&record, range)?;
            let organization =
                organizations
                    .remove(&record.session.id)
                    .unwrap_or(SessionOrganization {
                        project: None,
                        tags: Vec::new(),
                    });
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
                category: record.effective_category,
                window_title: record.session.window_title,
                note: record.session.note,
                project: organization.project,
                tags: organization.tags,
                started_at_ms: start,
                ended_at_ms: end,
                duration_ms: end - start,
                is_open: record.session.is_open,
            })
        })
        .collect()
}

fn organization_insights(entries: &[TimelineEntry]) -> OrganizationInsights {
    let mut project_usage = HashMap::<i64, OrganizationUsage>::new();
    let mut tag_usage = HashMap::<i64, OrganizationUsage>::new();
    let mut unassigned_active_duration_ms = 0;
    let mut unassigned_session_count = 0;

    for entry in entries {
        if entry.state != ActivityState::Active {
            continue;
        }

        if let Some(project) = &entry.project {
            let usage = project_usage
                .entry(project.id)
                .or_insert_with(|| OrganizationUsage {
                    id: project.id,
                    name: project.name.clone(),
                    color: project.color.clone(),
                    duration_ms: 0,
                    session_count: 0,
                });
            usage.duration_ms += entry.duration_ms;
            usage.session_count += 1;
        }

        for tag in &entry.tags {
            let usage = tag_usage
                .entry(tag.id)
                .or_insert_with(|| OrganizationUsage {
                    id: tag.id,
                    name: tag.name.clone(),
                    color: tag.color.clone(),
                    duration_ms: 0,
                    session_count: 0,
                });
            usage.duration_ms += entry.duration_ms;
            usage.session_count += 1;
        }

        if entry.project.is_none() && entry.tags.is_empty() {
            unassigned_active_duration_ms += entry.duration_ms;
            unassigned_session_count += 1;
        }
    }

    let mut project_usage = project_usage.into_values().collect::<Vec<_>>();
    let mut tag_usage = tag_usage.into_values().collect::<Vec<_>>();
    let compare_usage = |left: &OrganizationUsage, right: &OrganizationUsage| {
        right
            .duration_ms
            .cmp(&left.duration_ms)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    };
    project_usage.sort_by(compare_usage);
    tag_usage.sort_by(compare_usage);

    OrganizationInsights {
        project_usage,
        tag_usage,
        unassigned_active_duration_ms,
        unassigned_session_count,
    }
}

fn clipped_bounds(record: &ActivityRecord, range: TimeRange) -> Option<(i64, i64)> {
    let start = record.session.started_at_ms.max(range.start_ms);
    let end = record.session.ended_at_ms.min(range.end_ms);
    (end > start).then_some((start, end))
}

pub fn local_day_range(date: NaiveDate) -> AppResult<TimeRange> {
    local_date_range_inclusive(date, date)
}

pub fn local_date_range_inclusive(
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> AppResult<TimeRange> {
    if end_date < start_date {
        return Err(AppError::InvalidTimeRange(
            "end date must not be before start date".to_owned(),
        ));
    }
    let next_date = end_date
        .succ_opt()
        .ok_or_else(|| AppError::InvalidTimeRange("local date overflow".to_owned()))?;
    TimeRange::new(
        resolve_local_midnight(start_date)?,
        resolve_local_midnight(next_date)?,
    )
}

fn resolve_local_midnight(date: NaiveDate) -> AppResult<i64> {
    let midnight = date.and_time(NaiveTime::MIN);
    for minute in 0..=24 * 60 {
        let candidate = midnight
            .checked_add_signed(Duration::minutes(minute))
            .ok_or_else(|| AppError::InvalidTimeRange("local date overflow".to_owned()))?;
        match Local.from_local_datetime(&candidate) {
            LocalResult::Single(value) => return Ok(value.timestamp_millis()),
            LocalResult::Ambiguous(earliest, _) => return Ok(earliest.timestamp_millis()),
            LocalResult::None => {}
        }
    }
    Err(AppError::InvalidTimeRange(format!(
        "local date has no representable boundary for {date}"
    )))
}

#[cfg(test)]
mod tests {
    use crate::{
        activity::{ClosedReason, NewApplication, NewSession},
        database::{ActivityTagInput, Database, ProjectInput},
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
        store_session_with_category_override(
            repository,
            state,
            application_id,
            None,
            start_ms,
            end_ms,
        );
    }

    fn store_session_with_category_override(
        repository: &ActivityRepository,
        state: ActivityState,
        application_id: Option<i64>,
        category_override: Option<&str>,
        start_ms: i64,
        end_ms: i64,
    ) {
        let session = repository
            .create_session(&NewSession {
                state,
                application_id,
                window_title: None,
                category_override: category_override.map(str::to_owned),
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
    fn timeline_entries_include_session_organization() {
        let (service, repository, application_id) = setup();
        let project = repository
            .create_project(&ProjectInput {
                name: "Client launch".to_owned(),
                color: "#39796A".to_owned(),
            })
            .expect("project should be stored");
        let tag = repository
            .create_activity_tag(&ActivityTagInput {
                name: "Deep work".to_owned(),
                color: "#8B5CF6".to_owned(),
            })
            .expect("tag should be stored");
        let session = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(application_id),
                window_title: None,
                category_override: None,
                started_at_ms: 100,
            })
            .expect("session should open");
        repository
            .close_session(session.id, 200, ClosedReason::AppChanged)
            .expect("session should close");
        repository
            .set_session_organization(session.id, Some(project.id), &[tag.id])
            .expect("organization should be stored");

        let timeline = service
            .timeline(TimeRange::new(0, 300).expect("range should be valid"))
            .expect("timeline should load");

        assert_eq!(timeline.len(), 1);
        assert_eq!(
            timeline[0].project.as_ref().map(|item| item.id),
            Some(project.id)
        );
        assert_eq!(
            timeline[0]
                .tags
                .iter()
                .map(|item| item.id)
                .collect::<Vec<_>>(),
            vec![tag.id]
        );
    }

    #[test]
    fn filtered_timeline_page_reports_exact_totals_and_pagination() {
        let (service, repository, application_id) = setup();
        let date = NaiveDate::from_ymd_opt(2025, 1, 15).expect("date should be valid");
        let range = local_day_range(date).expect("local day should resolve");
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            range.start_ms + 60 * 60_000,
            range.start_ms + 70 * 60_000,
        );
        store_session(
            &repository,
            ActivityState::Idle,
            None,
            range.start_ms + 70 * 60_000,
            range.start_ms + 75 * 60_000,
        );
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            range.start_ms + 80 * 60_000,
            range.start_ms + 95 * 60_000,
        );
        let search = TimelineSearch {
            query: Some("idea".to_owned()),
            ..TimelineSearch::default()
        };

        let first = service
            .timeline_page_for_date_filtered(date, 0, 1, &search)
            .expect("first filtered page should load");
        assert_eq!(first.total_count, 2);
        assert_eq!(first.entries.len(), 1);
        assert_eq!(first.active_duration_ms, 25 * 60_000);
        assert_eq!(first.idle_duration_ms, 0);
        assert!(first.has_more);

        let second = service
            .timeline_page_for_date_filtered(date, 1, 1, &search)
            .expect("second filtered page should load");
        assert_eq!(second.total_count, 2);
        assert_eq!(second.entries.len(), 1);
        assert!(!second.has_more);
    }

    #[test]
    fn filtered_timeline_date_range_includes_both_endpoint_days() {
        let (service, repository, application_id) = setup();
        let first_date = NaiveDate::from_ymd_opt(2025, 1, 15).expect("date should be valid");
        let second_date = first_date.succ_opt().expect("next date should exist");
        let third_date = second_date.succ_opt().expect("third date should exist");
        let first_range = local_day_range(first_date).expect("first local day should resolve");
        let second_range = local_day_range(second_date).expect("second local day should resolve");
        let third_range = local_day_range(third_date).expect("third local day should resolve");
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            first_range.end_ms - 10 * 60_000,
            second_range.start_ms + 10 * 60_000,
        );
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            second_range.start_ms + 60 * 60_000,
            second_range.start_ms + 75 * 60_000,
        );
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            third_range.start_ms + 60 * 60_000,
            third_range.start_ms + 90 * 60_000,
        );
        let search = TimelineSearch {
            query: Some("idea".to_owned()),
            ..TimelineSearch::default()
        };

        let first_page = service
            .timeline_page_for_date_range_filtered(first_date, second_date, 0, 1, &search)
            .expect("first range page should load");
        assert_eq!(first_page.total_count, 2);
        assert_eq!(first_page.active_duration_ms, 35 * 60_000);
        assert_eq!(first_page.entries.len(), 1);
        assert_eq!(
            first_page.entries[0].started_at_ms,
            second_range.start_ms + 60 * 60_000
        );
        assert!(first_page.has_more);

        let second_page = service
            .timeline_page_for_date_range_filtered(first_date, second_date, 1, 1, &search)
            .expect("second range page should load");
        assert_eq!(second_page.entries.len(), 1);
        assert_eq!(
            second_page.entries[0].started_at_ms,
            first_range.end_ms - 10 * 60_000
        );
        assert!(!second_page.has_more);
    }

    #[test]
    fn local_date_range_rejects_reversed_dates() {
        let start = NaiveDate::from_ymd_opt(2025, 1, 16).expect("date should be valid");
        let end = start.pred_opt().expect("previous date should exist");
        assert!(matches!(
            local_date_range_inclusive(start, end),
            Err(AppError::InvalidTimeRange(_))
        ));
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
    fn effective_categories_do_not_replace_the_application_base_category() {
        let (service, repository, application_id) = setup();
        repository
            .update_application_preferences(application_id, "Base category", false, false)
            .expect("base category should update");
        store_session_with_category_override(
            &repository,
            ActivityState::Active,
            Some(application_id),
            Some("Rule A"),
            100,
            200,
        );
        store_session_with_category_override(
            &repository,
            ActivityState::Active,
            Some(application_id),
            Some("Rule B"),
            200,
            350,
        );
        let range = TimeRange::new(0, 500).expect("range should be valid");

        let applications = service
            .app_usage(range)
            .expect("application usage should load");
        assert_eq!(applications.len(), 1);
        assert_eq!(applications[0].category, "Base category");

        let timeline = service.timeline(range).expect("timeline should load");
        assert_eq!(timeline[0].category.as_deref(), Some("Rule A"));
        assert_eq!(timeline[1].category.as_deref(), Some("Rule B"));

        let categories = service
            .category_usage(range)
            .expect("category usage should load");
        assert_eq!(
            categories,
            vec![
                CategoryUsage {
                    category: "Rule B".to_owned(),
                    duration_ms: 150,
                    application_count: 1,
                },
                CategoryUsage {
                    category: "Rule A".to_owned(),
                    duration_ms: 100,
                    application_count: 1,
                },
            ]
        );

        let limit = repository
            .create_usage_limit(
                &crate::database::UsageLimitRuleInput {
                    scope_type: crate::database::UsageLimitScopeType::Category,
                    application_id: None,
                    category: Some("Rule A".to_owned()),
                    weekday_limit_minutes: 60,
                    weekend_limit_minutes: 60,
                    notifications_enabled: true,
                    enabled: true,
                },
                500,
            )
            .expect("category limit should be stored");
        assert_eq!(
            repository
                .active_usage_duration_for_rule(&limit, range.start_ms, range.end_ms)
                .expect("category usage limit duration should load"),
            100
        );
    }

    #[test]
    fn productivity_report_compares_the_previous_equal_range() {
        let (service, repository, application_id) = setup();
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            0,
            50,
        );
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            100,
            200,
        );
        store_session(&repository, ActivityState::Idle, None, 200, 250);

        let report = service
            .productivity_report(TimeRange::new(100, 300).unwrap())
            .unwrap();
        assert_eq!(report.active_duration_ms, 100);
        assert_eq!(report.idle_duration_ms, 50);
        assert_eq!(report.previous_active_duration_ms, 50);
        assert_eq!(report.hourly_usage.len(), 24);
        assert_eq!(
            report
                .daily_usage
                .iter()
                .map(|day| day.active_duration_ms)
                .sum::<i64>(),
            100
        );
        assert_eq!(
            report
                .daily_usage
                .iter()
                .map(|day| day.idle_duration_ms)
                .sum::<i64>(),
            50
        );
        assert_eq!(
            report
                .category_usage
                .iter()
                .map(|category| category.duration_ms)
                .sum::<i64>(),
            100
        );
    }

    #[test]
    fn productivity_report_aggregates_clipped_active_organization_insights() {
        let (service, repository, application_id) = setup();
        let beta = repository
            .create_project(&ProjectInput {
                name: "Beta".to_owned(),
                color: "#39796A".to_owned(),
            })
            .expect("project should be stored");
        let alpha = repository
            .create_project(&ProjectInput {
                name: "Alpha".to_owned(),
                color: "#8B5CF6".to_owned(),
            })
            .expect("project should be stored");
        let focus = repository
            .create_activity_tag(&ActivityTagInput {
                name: "Focus".to_owned(),
                color: "#0F766E".to_owned(),
            })
            .expect("tag should be stored");
        let overlap = repository
            .create_activity_tag(&ActivityTagInput {
                name: "Overlap".to_owned(),
                color: "#F97316".to_owned(),
            })
            .expect("tag should be stored");

        let clipped = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(application_id),
                window_title: None,
                category_override: None,
                started_at_ms: 50,
            })
            .expect("session should open");
        repository
            .close_session(clipped.id, 250, ClosedReason::AppChanged)
            .expect("session should close");
        repository
            .set_session_organization(clipped.id, Some(beta.id), &[focus.id, overlap.id])
            .expect("organization should be stored");

        let alpha_session = repository
            .create_session(&NewSession {
                state: ActivityState::Active,
                application_id: Some(application_id),
                window_title: None,
                category_override: None,
                started_at_ms: 100,
            })
            .expect("session should open");
        repository
            .close_session(alpha_session.id, 200, ClosedReason::AppChanged)
            .expect("session should close");
        repository
            .set_session_organization(alpha_session.id, Some(alpha.id), &[focus.id])
            .expect("organization should be stored");

        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            150,
            300,
        );

        let idle = repository
            .create_session(&NewSession {
                state: ActivityState::Idle,
                application_id: None,
                window_title: None,
                category_override: None,
                started_at_ms: 100,
            })
            .expect("idle session should open");
        repository
            .close_session(idle.id, 200, ClosedReason::BecameActive)
            .expect("idle session should close");
        repository
            .set_session_organization(idle.id, Some(alpha.id), &[focus.id])
            .expect("idle organization should be stored");

        repository
            .set_project_archived(beta.id, true)
            .expect("project should archive");
        repository
            .set_activity_tag_archived(overlap.id, true)
            .expect("tag should archive");

        let report = service
            .productivity_report(TimeRange::new(100, 200).expect("range should be valid"))
            .expect("report should load");

        assert_eq!(
            report.organization_insights.project_usage,
            vec![
                OrganizationUsage {
                    id: alpha.id,
                    name: "Alpha".to_owned(),
                    color: alpha.color,
                    duration_ms: 100,
                    session_count: 1,
                },
                OrganizationUsage {
                    id: beta.id,
                    name: "Beta".to_owned(),
                    color: beta.color,
                    duration_ms: 100,
                    session_count: 1,
                },
            ]
        );
        assert_eq!(
            report.organization_insights.tag_usage,
            vec![
                OrganizationUsage {
                    id: focus.id,
                    name: "Focus".to_owned(),
                    color: focus.color,
                    duration_ms: 200,
                    session_count: 2,
                },
                OrganizationUsage {
                    id: overlap.id,
                    name: "Overlap".to_owned(),
                    color: overlap.color,
                    duration_ms: 100,
                    session_count: 1,
                },
            ]
        );
        assert_eq!(
            (
                report.organization_insights.unassigned_active_duration_ms,
                report.organization_insights.unassigned_session_count,
            ),
            (50, 1)
        );
    }

    #[test]
    fn category_usage_aggregates_duration_and_unique_applications() {
        let (service, repository, application_id) = setup();
        repository
            .update_application_preferences(application_id, "Work", false, false)
            .expect("first application category should update");
        let second = repository
            .upsert_application(&NewApplication {
                name: "Terminal".to_owned(),
                bundle_id: Some("example.terminal".to_owned()),
                executable_path: None,
                seen_at_ms: 0,
            })
            .expect("second application should be stored");
        repository
            .update_application_preferences(second.id, "Work", false, false)
            .expect("second application category should update");
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
        store_session(
            &repository,
            ActivityState::Active,
            Some(second.id),
            450,
            650,
        );

        let usage = service
            .category_usage(TimeRange::new(0, 1_000).expect("range should be valid"))
            .expect("category usage should succeed");
        assert_eq!(
            usage,
            vec![CategoryUsage {
                category: "Work".to_owned(),
                duration_ms: 450,
                application_count: 2,
            }]
        );
    }

    #[test]
    fn focus_summary_groups_short_breaks_and_counts_application_switches() {
        let (service, repository, application_id) = setup();
        let second = repository
            .upsert_application(&NewApplication {
                name: "Terminal".to_owned(),
                bundle_id: Some("example.terminal".to_owned()),
                executable_path: None,
                seen_at_ms: 0,
            })
            .expect("second application should be stored");
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            0,
            100,
        );
        store_session(
            &repository,
            ActivityState::Active,
            Some(second.id),
            100,
            200,
        );
        store_session(&repository, ActivityState::Idle, None, 200, 250);
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            250,
            350,
        );
        store_session(&repository, ActivityState::Idle, None, 350, 70_000);
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            70_000,
            70_150,
        );

        let summary = service
            .focus_summary(
                TimeRange::new(0, 100_000).expect("range should be valid"),
                1,
                240,
            )
            .expect("focus summary should succeed");
        assert_eq!(summary.blocks.len(), 2);
        assert_eq!(summary.total_focus_duration_ms, 450);
        assert_eq!(summary.longest_focus_duration_ms, 300);
        assert_eq!(summary.application_switch_count, 2);
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
    fn usage_limit_progress_uses_local_day_clipping_and_weekend_limits() {
        let (service, repository, application_id) = setup();
        repository
            .create_usage_limit(
                &crate::database::UsageLimitRuleInput {
                    scope_type: UsageLimitScopeType::Application,
                    application_id: Some(application_id),
                    category: None,
                    weekday_limit_minutes: 1,
                    weekend_limit_minutes: 2,
                    notifications_enabled: true,
                    enabled: true,
                },
                0,
            )
            .expect("rule should be created");
        let monday = NaiveDate::from_ymd_opt(2025, 1, 13).expect("Monday should exist");
        let monday_range = local_day_range(monday).expect("Monday range should resolve");
        store_session(
            &repository,
            ActivityState::Active,
            Some(application_id),
            monday_range.start_ms - 12_000,
            monday_range.start_ms + 48_000,
        );
        let weekday = service
            .usage_limit_progress_for_date(monday)
            .expect("weekday progress should load")
            .pop()
            .expect("rule should have progress");
        assert_eq!(weekday.limit_minutes, 1);
        assert_eq!(weekday.used_duration_ms, 48_000);
        assert_eq!(weekday.percentage, 80.0);
        assert_eq!(weekday.threshold_state, UsageLimitThresholdState::Reached80);

        let saturday = NaiveDate::from_ymd_opt(2025, 1, 18).expect("Saturday should exist");
        let rule_id = repository
            .usage_limit_rules()
            .expect("usage rules should load")
            .pop()
            .expect("usage rule should exist")
            .id;
        repository
            .add_temporary_usage_limit_minutes(rule_id, &saturday.to_string(), 30, 0)
            .expect("weekend exception should be stored");
        let weekend = service
            .usage_limit_progress_for_date(saturday)
            .expect("weekend progress should load")
            .pop()
            .expect("rule should have progress");
        assert_eq!(weekend.base_limit_minutes, 2);
        assert_eq!(weekend.temporary_added_minutes, 30);
        assert_eq!(weekend.limit_minutes, 32);
        assert_eq!(weekend.used_duration_ms, 0);
        assert_eq!(weekend.threshold_state, UsageLimitThresholdState::Below80);
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
