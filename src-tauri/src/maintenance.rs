use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use chrono::{
    DateTime, Datelike, Duration as ChronoDuration, Local, NaiveDate, NaiveDateTime, Timelike, Utc,
};

use crate::{
    database::{ActivityRepository, MaintenanceResult, WeeklyReportArchiveInput},
    statistics::{StatisticsService, TimeRange},
};

const DAY_MS: i64 = 24 * 60 * 60 * 1_000;

#[derive(Debug, Clone)]
pub struct WeeklyArchiveNotification {
    pub week_start_date: String,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaintenanceStatus {
    pub running: bool,
    pub last_success_at_ms: Option<i64>,
    pub last_error: Option<String>,
}

pub fn archive_due_weekly_report(
    repository: &ActivityRepository,
    statistics: &StatisticsService,
    now_ms: i64,
) -> Result<Option<WeeklyArchiveNotification>, String> {
    let settings = repository.settings().map_err(|error| error.to_string())?;
    if !settings.weekly_report_auto_archive_enabled {
        return Ok(None);
    }
    let now = DateTime::<Utc>::from_timestamp_millis(now_ms)
        .ok_or_else(|| "weekly report timestamp is outside the supported range".to_owned())?
        .with_timezone(&Local);
    let current_monday = now
        .date_naive()
        .checked_sub_signed(ChronoDuration::days(
            now.weekday().num_days_from_monday() as i64
        ))
        .ok_or_else(|| "weekly report date is outside the supported range".to_owned())?;
    let week_start = current_monday
        .checked_sub_signed(ChronoDuration::days(7))
        .ok_or_else(|| "weekly report date is outside the supported range".to_owned())?;
    let week_end = current_monday
        .checked_sub_signed(ChronoDuration::days(1))
        .ok_or_else(|| "weekly report date is outside the supported range".to_owned())?;
    let week_start_text = week_start.format("%Y-%m-%d").to_string();

    let start_range =
        crate::statistics::local_day_range(week_start).map_err(|error| error.to_string())?;
    let end_range =
        crate::statistics::local_day_range(week_end).map_err(|error| error.to_string())?;
    let existing = repository
        .weekly_report_archive(&week_start_text)
        .map_err(|error| error.to_string())?;
    let needs_final_archive =
        existing.is_none_or(|archive| archive.generated_at_ms < end_range.end_ms);
    if needs_final_archive {
        let range = TimeRange::new(start_range.start_ms, end_range.end_ms)
            .map_err(|error| error.to_string())?;
        let report = statistics
            .productivity_report(range)
            .map_err(|error| error.to_string())?;
        let strongest_day_date = report
            .daily_usage
            .iter()
            .max_by_key(|day| day.active_duration_ms)
            .filter(|day| day.active_duration_ms > 0)
            .map(|day| day.date.clone());
        let peak_hour = report
            .hourly_usage
            .iter()
            .max_by_key(|hour| hour.active_duration_ms)
            .filter(|hour| hour.active_duration_ms > 0)
            .map(|hour| hour.hour as i64);
        let leading_category = report
            .category_usage
            .iter()
            .max_by_key(|category| category.duration_ms)
            .filter(|category| category.duration_ms > 0)
            .map(|category| category.category.clone());
        let plans = repository
            .focus_plan_history(range.start_ms, range.end_ms)
            .map_err(|error| error.to_string())?;
        let completed = plans
            .iter()
            .filter(|plan| plan.outcome == "COMPLETED")
            .count();
        let focus_completion_rate =
            (!plans.is_empty()).then(|| (completed.saturating_mul(100) / plans.len()) as i64);
        let input = WeeklyReportArchiveInput {
            week_start_date: week_start_text.clone(),
            week_end_date: week_end.format("%Y-%m-%d").to_string(),
            generated_at_ms: now_ms,
            active_duration_ms: report.active_duration_ms,
            idle_duration_ms: report.idle_duration_ms,
            previous_week_active_duration_ms: report.previous_active_duration_ms,
            strongest_day_date,
            peak_hour,
            leading_category,
            focus_completion_rate,
            payload_json: serde_json::to_string(&report).map_err(|error| error.to_string())?,
        };
        repository
            .archive_weekly_report(&input)
            .map_err(|error| error.to_string())?;
    }

    if !settings.weekly_report_notification_enabled {
        return Ok(None);
    }

    let scheduled_minutes = parse_clock_minutes(&settings.weekly_report_notification_time)?;
    let Some(archive) = repository
        .oldest_unnotified_weekly_report_archive()
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let should_notify = weekly_notification_is_due(
        &now,
        &archive.week_end_date,
        settings.weekly_report_notification_weekday,
        scheduled_minutes,
    )?;
    Ok(should_notify.then_some(WeeklyArchiveNotification {
        week_start_date: archive.week_start_date,
    }))
}

fn weekly_notification_is_due(
    now: &DateTime<Local>,
    archive_week_end_date: &str,
    scheduled_weekday: i64,
    scheduled_minutes: i64,
) -> Result<bool, String> {
    let week_end = NaiveDate::parse_from_str(archive_week_end_date, "%Y-%m-%d")
        .map_err(|_| "weekly report archive has an invalid end date".to_owned())?;
    let scheduled_date = week_end
        .checked_add_signed(ChronoDuration::days(scheduled_weekday))
        .ok_or_else(|| {
            "weekly report notification date is outside the supported range".to_owned()
        })?;
    let current_minutes = now.hour() as i64 * 60 + now.minute() as i64;
    Ok(now.date_naive() > scheduled_date
        || (now.date_naive() == scheduled_date && current_minutes >= scheduled_minutes))
}

fn parse_clock_minutes(value: &str) -> Result<i64, String> {
    let (hours, minutes) = value
        .split_once(':')
        .ok_or_else(|| "weekly report notification time must use HH:MM".to_owned())?;
    let hours = hours
        .parse::<i64>()
        .map_err(|_| "invalid hour".to_owned())?;
    let minutes = minutes
        .parse::<i64>()
        .map_err(|_| "invalid minute".to_owned())?;
    Ok(hours * 60 + minutes)
}

#[derive(Clone, Default)]
pub struct MaintenanceStatusState(Arc<Mutex<MaintenanceStatus>>);

impl MaintenanceStatusState {
    pub fn snapshot(&self) -> MaintenanceStatus {
        self.0
            .lock()
            .map(|status| status.clone())
            .unwrap_or_else(|_| MaintenanceStatus {
                running: false,
                last_success_at_ms: None,
                last_error: Some("maintenance status lock was poisoned".to_owned()),
            })
    }

    pub fn start(&self) {
        if let Ok(mut status) = self.0.lock() {
            status.running = true;
            status.last_error = None;
        }
    }

    pub fn finish(&self, now_ms: i64, result: &Result<(), String>) {
        if let Ok(mut status) = self.0.lock() {
            status.running = false;
            match result {
                Ok(()) => {
                    status.last_success_at_ms = Some(now_ms);
                    status.last_error = None;
                }
                Err(error) => status.last_error = Some(error.clone()),
            }
        }
    }
}

pub fn run_due(
    repository: &ActivityRepository,
    app_data: &Path,
    now_ms: i64,
) -> Result<(), String> {
    let settings = repository.settings().map_err(|error| error.to_string())?;
    if now_ms.saturating_sub(settings.last_maintenance_at_ms) >= DAY_MS {
        run_cleanup(repository, app_data, now_ms, settings.retention_days)?;
    }
    if settings.automatic_backup_enabled {
        let interval_ms = if settings.backup_interval == "DAILY" {
            DAY_MS
        } else {
            7 * DAY_MS
        };
        if now_ms.saturating_sub(settings.last_backup_at_ms) >= interval_ms {
            create_automatic_backup(repository, app_data, now_ms)?;
        }
    }
    if settings.automatic_encrypted_backup_enabled {
        let interval_ms = if settings.backup_interval == "DAILY" {
            DAY_MS
        } else {
            7 * DAY_MS
        };
        if now_ms.saturating_sub(settings.last_encrypted_backup_at_ms) >= interval_ms {
            create_automatic_encrypted_backup(repository, app_data, now_ms)?;
        }
    }
    Ok(())
}

pub fn create_automatic_encrypted_backup(
    repository: &ActivityRepository,
    app_data: &Path,
    now_ms: i64,
) -> Result<PathBuf, String> {
    let settings = repository.settings().map_err(|error| error.to_string())?;
    let directory = settings
        .backup_directory
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| app_data.join("backups"));
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let timestamp = DateTime::<Utc>::from_timestamp_millis(now_ms)
        .ok_or_else(|| "backup timestamp is outside the supported range".to_owned())?
        .format("%Y%m%d-%H%M%S");
    let destination = directory.join(format!("watchhouse-auto-{timestamp}.yqbackup"));
    crate::commands::encrypted_backup::create_automatic_encrypted_backup(repository, &destination)?;
    repository
        .mark_encrypted_backup_completed(now_ms)
        .map_err(|error| error.to_string())?;
    prune_backups_with_extension(&directory, settings.backup_keep_count as usize, ".yqbackup")?;
    Ok(destination)
}

pub fn run_cleanup(
    repository: &ActivityRepository,
    app_data: &Path,
    now_ms: i64,
    retention_days: i64,
) -> Result<MaintenanceResult, String> {
    let result = repository
        .run_maintenance(now_ms, retention_days)
        .map_err(|error| error.to_string())?;
    remove_application_icons(app_data, &result.deleted_application_ids);
    Ok(result)
}

pub fn create_automatic_backup(
    repository: &ActivityRepository,
    app_data: &Path,
    now_ms: i64,
) -> Result<PathBuf, String> {
    let settings = repository.settings().map_err(|error| error.to_string())?;
    let directory = settings
        .backup_directory
        .filter(|path| !path.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| app_data.join("backups"));
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let timestamp = DateTime::<Utc>::from_timestamp_millis(now_ms)
        .ok_or_else(|| "backup timestamp is outside the supported range".to_owned())?
        .format("%Y%m%d-%H%M%S");
    let destination = directory.join(format!("watchhouse-auto-{timestamp}.sqlite3"));
    repository
        .backup_database(&destination)
        .map_err(|error| error.to_string())?;
    repository
        .mark_backup_completed(now_ms)
        .map_err(|error| error.to_string())?;
    prune_backups(&directory, settings.backup_keep_count as usize)?;
    Ok(destination)
}

fn remove_application_icons(app_data: &Path, application_ids: &[i64]) {
    let directory = app_data.join("icons");
    for application_id in application_ids {
        for extension in ["png", "revision"] {
            let _ = fs::remove_file(directory.join(format!("{application_id}.{extension}")));
        }
    }
}

fn prune_backups(directory: &Path, keep_count: usize) -> Result<(), String> {
    prune_backups_with_extension(directory, keep_count, ".sqlite3")
}

fn prune_backups_with_extension(
    directory: &Path,
    keep_count: usize,
    extension: &str,
) -> Result<(), String> {
    let mut backups = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_ok_and(|file_type| file_type.is_file())
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| is_automatic_backup_name(name, extension))
        })
        .collect::<Vec<_>>();
    backups.sort_by_key(|entry| std::cmp::Reverse(entry.file_name()));
    for entry in backups.into_iter().skip(keep_count) {
        fs::remove_file(entry.path()).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn is_automatic_backup_name(name: &str, extension: &str) -> bool {
    name.strip_prefix("watchhouse-auto-")
        .and_then(|value| value.strip_suffix(extension))
        .is_some_and(|timestamp| {
            timestamp.len() == 15
                && timestamp.as_bytes().get(8) == Some(&b'-')
                && NaiveDateTime::parse_from_str(timestamp, "%Y%m%d-%H%M%S").is_ok()
        })
}

#[cfg(test)]
mod tests {
    use chrono::{LocalResult, TimeZone};

    use crate::database::Database;

    use super::*;

    fn weekly_report_setup() -> (ActivityRepository, StatisticsService) {
        let repository =
            ActivityRepository::new(Database::in_memory().expect("database should initialize"));
        let mut settings = repository.settings().expect("settings should load");
        settings.weekly_report_auto_archive_enabled = true;
        settings.weekly_report_notification_enabled = true;
        settings.weekly_report_notification_weekday = 2;
        settings.weekly_report_notification_time = "09:00".to_owned();
        repository
            .update_settings(&settings, 1)
            .expect("settings should update");
        let statistics = StatisticsService::new(repository.clone());
        (repository, statistics)
    }

    fn local_timestamp(year: i32, month: u32, day: u32, hour: u32, minute: u32) -> i64 {
        match Local.with_ymd_and_hms(year, month, day, hour, minute, 0) {
            LocalResult::Single(value) => value.timestamp_millis(),
            LocalResult::Ambiguous(earliest, _) => earliest.timestamp_millis(),
            LocalResult::None => panic!("test timestamp should exist in the local timezone"),
        }
    }

    #[test]
    fn weekly_report_notification_is_due_at_the_scheduled_time() {
        let (repository, statistics) = weekly_report_setup();

        let notification =
            archive_due_weekly_report(&repository, &statistics, local_timestamp(2026, 8, 4, 9, 0))
                .expect("weekly report archive should succeed")
                .expect("notification should be due");

        assert_eq!(notification.week_start_date, "2026-07-27");
    }

    #[test]
    fn weekly_report_notification_is_caught_up_after_the_scheduled_day() {
        let (repository, statistics) = weekly_report_setup();

        let before_schedule =
            archive_due_weekly_report(&repository, &statistics, local_timestamp(2026, 8, 4, 8, 59))
                .expect("weekly report archive should succeed");
        assert!(before_schedule.is_none());

        let notification = archive_due_weekly_report(
            &repository,
            &statistics,
            local_timestamp(2026, 8, 10, 15, 30),
        )
        .expect("weekly report archive should succeed")
        .expect("missed notification should be caught up");

        assert_eq!(notification.week_start_date, "2026-07-27");
    }

    #[test]
    fn weekly_report_notification_waits_until_the_scheduled_time() {
        let (repository, statistics) = weekly_report_setup();

        let notification =
            archive_due_weekly_report(&repository, &statistics, local_timestamp(2026, 8, 4, 8, 59))
                .expect("weekly report archive should succeed");

        assert!(notification.is_none());
        assert!(
            repository
                .weekly_report_archive("2026-07-27")
                .expect("archive lookup should succeed")
                .is_some()
        );
    }

    #[test]
    fn automatic_archive_replaces_a_partial_snapshot_after_the_week_ends() {
        let (repository, statistics) = weekly_report_setup();
        let partial_created_at_ms = local_timestamp(2026, 7, 29, 12, 0);
        repository
            .archive_weekly_report(&WeeklyReportArchiveInput {
                week_start_date: "2026-07-27".to_owned(),
                week_end_date: "2026-08-02".to_owned(),
                generated_at_ms: partial_created_at_ms,
                active_duration_ms: 1,
                idle_duration_ms: 0,
                previous_week_active_duration_ms: 0,
                strongest_day_date: None,
                peak_hour: None,
                leading_category: None,
                focus_completion_rate: None,
                payload_json: "{}".to_owned(),
            })
            .expect("partial archive should save");

        let finalized_at_ms = local_timestamp(2026, 8, 3, 8, 0);
        archive_due_weekly_report(&repository, &statistics, finalized_at_ms)
            .expect("automatic archive should succeed");

        let archive = repository
            .weekly_report_archive("2026-07-27")
            .expect("archive lookup should succeed")
            .expect("archive should exist");
        assert_eq!(archive.generated_at_ms, finalized_at_ms);
        assert_eq!(archive.active_duration_ms, 0);
    }

    #[test]
    fn weekly_report_notification_is_not_repeated_after_being_marked() {
        let (repository, statistics) = weekly_report_setup();
        archive_due_weekly_report(&repository, &statistics, local_timestamp(2026, 8, 4, 9, 0))
            .expect("weekly report archive should succeed")
            .expect("notification should initially be due");
        repository
            .mark_weekly_report_notified("2026-07-27", local_timestamp(2026, 8, 4, 9, 1))
            .expect("archive should be marked as notified");

        let notification = archive_due_weekly_report(
            &repository,
            &statistics,
            local_timestamp(2026, 8, 7, 15, 30),
        )
        .expect("weekly report archive should succeed");

        assert!(notification.is_none());
    }

    #[test]
    fn automatic_backups_are_rotated_and_completion_is_recorded() {
        let repository =
            ActivityRepository::new(Database::in_memory().expect("database should initialize"));
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let mut settings = repository.settings().expect("settings should load");
        settings.backup_directory = Some(directory.path().to_string_lossy().into_owned());
        settings.backup_keep_count = 2;
        repository
            .update_settings(&settings, 1)
            .expect("settings should update");

        for timestamp in [1_000, 2_000, 3_000] {
            create_automatic_backup(&repository, directory.path(), timestamp)
                .expect("backup should succeed");
        }

        let backups = fs::read_dir(directory.path())
            .expect("backup directory should be readable")
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with("watchhouse-auto-"))
            })
            .count();
        assert_eq!(backups, 2);
        assert_eq!(
            repository
                .settings()
                .expect("settings should reload")
                .last_backup_at_ms,
            3_000
        );
    }

    #[test]
    fn backup_rotation_ignores_files_that_only_resemble_generated_backups() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let names = [
            "watchhouse-auto-not-a-date.sqlite3",
            "watchhouse-auto-20260230-120000.sqlite3",
            "watchhouse-auto-20260101-120000-copy.sqlite3",
            "watchhouse-auto-20260101-120000.sqlite3.bak",
            "watchhouse-auto-not-a-date.yqbackup",
            "watchhouse-auto-20260230-120000.yqbackup",
            "watchhouse-auto-20260101-120000-copy.yqbackup",
            "watchhouse-auto-20260101-120000.yqbackup.bak",
        ];
        for name in names {
            fs::write(directory.path().join(name), b"keep")
                .expect("test backup file should be written");
        }

        prune_backups(directory.path(), 0).expect("backup rotation should succeed");
        prune_backups_with_extension(directory.path(), 0, ".yqbackup")
            .expect("encrypted backup rotation should succeed");

        for name in names {
            assert!(
                directory.path().join(name).exists(),
                "{name} should be preserved"
            );
        }
    }

    #[test]
    fn backup_rotation_removes_only_old_strictly_named_files_for_each_format() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        for extension in [".sqlite3", ".yqbackup"] {
            for timestamp in ["20260101-120000", "20260102-120000", "20260103-120000"] {
                fs::write(
                    directory
                        .path()
                        .join(format!("watchhouse-auto-{timestamp}{extension}")),
                    b"backup",
                )
                .expect("test backup file should be written");
            }
            prune_backups_with_extension(directory.path(), 2, extension)
                .expect("backup rotation should succeed");

            assert!(
                !directory
                    .path()
                    .join(format!("watchhouse-auto-20260101-120000{extension}"))
                    .exists()
            );
            assert!(
                directory
                    .path()
                    .join(format!("watchhouse-auto-20260102-120000{extension}"))
                    .exists()
            );
            assert!(
                directory
                    .path()
                    .join(format!("watchhouse-auto-20260103-120000{extension}"))
                    .exists()
            );
        }
    }
}
