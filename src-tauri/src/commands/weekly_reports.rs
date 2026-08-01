use tauri::{AppHandle, Manager, State};
use tauri_plugin_notification::{NotificationExt, PermissionState};

use crate::{
    AppLocaleState,
    database::{ActivityRepository, WeeklyReportArchive, WeeklyReportArchiveInput},
};

use super::{IpcResult, run_blocking};

#[tauri::command]
pub async fn archive_weekly_report(
    input: WeeklyReportArchiveInput,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<WeeklyReportArchive> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.archive_weekly_report(&input)).await
}

#[tauri::command]
pub async fn get_weekly_report_archives(
    limit: usize,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<Vec<WeeklyReportArchive>> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.weekly_report_archives(limit)).await
}

#[tauri::command]
pub async fn delete_weekly_report_archive(
    week_start_date: String,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<()> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.delete_weekly_report_archive(&week_start_date)).await
}

#[tauri::command]
pub async fn send_weekly_report_notification(
    week_start_date: String,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> Result<(), String> {
    if app
        .notification()
        .permission_state()
        .map_err(|error| error.to_string())?
        != PermissionState::Granted
    {
        return Err("notification permission has not been granted".to_owned());
    }
    let archive_repository = repository.inner().clone();
    let lookup_date = week_start_date.clone();
    let archive = tauri::async_runtime::spawn_blocking(move || {
        archive_repository.weekly_report_archive(&lookup_date)
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())?
    .ok_or_else(|| "weekly report archive was not found".to_owned())?;

    let chinese = app.state::<AppLocaleState>().is_chinese();
    let duration = format_duration(archive.active_duration_ms);
    let category = archive
        .leading_category
        .as_deref()
        .filter(|category| !category.trim().is_empty());
    let (title, body) = if chinese {
        (
            "Watchhouse 周报已归档",
            category.map_or_else(
                || {
                    format!(
                        "{} 至 {} · 活跃 {duration}",
                        archive.week_start_date, archive.week_end_date
                    )
                },
                |category| {
                    format!(
                        "{} 至 {} · 活跃 {duration} · {category}",
                        archive.week_start_date, archive.week_end_date
                    )
                },
            ),
        )
    } else {
        (
            "Watchhouse weekly report archived",
            category.map_or_else(
                || {
                    format!(
                        "{} to {} · {duration} active",
                        archive.week_start_date, archive.week_end_date
                    )
                },
                |category| {
                    format!(
                        "{} to {} · {duration} active · {category}",
                        archive.week_start_date, archive.week_end_date
                    )
                },
            ),
        )
    };
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()
        .map_err(|error| error.to_string())?;
    repository
        .mark_weekly_report_notified(&week_start_date, now_ms()?)
        .map_err(|error| error.to_string())
}

fn format_duration(duration_ms: i64) -> String {
    let total_minutes = duration_ms.max(0) / 60_000;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    match (hours, minutes) {
        (0, minutes) => format!("{minutes}min"),
        (hours, 0) => format!("{hours}h"),
        (hours, minutes) => format!("{hours}h {minutes}min"),
    }
}

fn now_ms() -> Result<i64, String> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .map_err(|error| error.to_string())
}
