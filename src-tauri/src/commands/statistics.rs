use chrono::NaiveDate;
use std::fs;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    database::ActivityRepository,
    error::AppError,
    statistics::{
        AppUsage, CategoryUsage, DailyUsage, FocusSummary, ProductivityReport, StatisticsService,
        TimeRange, TimelineEntry, TimelinePage, TodaySummary,
    },
};

use super::{IpcResult, run_blocking};

#[tauri::command]
pub async fn get_today_summary(
    statistics: State<'_, StatisticsService>,
) -> IpcResult<TodaySummary> {
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.today_summary()).await
}

#[tauri::command]
pub async fn get_today_focus_summary(
    statistics: State<'_, StatisticsService>,
) -> IpcResult<FocusSummary> {
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.today_focus_summary()).await
}

#[tauri::command]
pub async fn get_timeline(
    date: String,
    statistics: State<'_, StatisticsService>,
) -> IpcResult<Vec<TimelineEntry>> {
    let date = parse_date(&date)?;
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.timeline_for_date(date)).await
}

#[tauri::command]
pub async fn get_timeline_page(
    date: String,
    offset: usize,
    limit: usize,
    statistics: State<'_, StatisticsService>,
) -> IpcResult<TimelinePage> {
    let date = parse_date(&date)?;
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.timeline_page_for_date(date, offset, limit)).await
}

#[tauri::command]
pub async fn get_app_usage(
    range_start_ms: i64,
    range_end_ms: i64,
    statistics: State<'_, StatisticsService>,
) -> IpcResult<Vec<AppUsage>> {
    let range = TimeRange::new(range_start_ms, range_end_ms)?;
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.app_usage(range)).await
}

#[tauri::command]
pub async fn get_category_usage(
    range_start_ms: i64,
    range_end_ms: i64,
    statistics: State<'_, StatisticsService>,
) -> IpcResult<Vec<CategoryUsage>> {
    let range = TimeRange::new(range_start_ms, range_end_ms)?;
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.category_usage(range)).await
}

#[tauri::command]
pub async fn get_daily_usage(
    range_start_ms: i64,
    range_end_ms: i64,
    statistics: State<'_, StatisticsService>,
) -> IpcResult<Vec<DailyUsage>> {
    let range = TimeRange::new(range_start_ms, range_end_ms)?;
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.daily_usage(range)).await
}

#[tauri::command]
pub async fn get_productivity_report(
    range_start_ms: i64,
    range_end_ms: i64,
    statistics: State<'_, StatisticsService>,
) -> IpcResult<ProductivityReport> {
    let range = TimeRange::new(range_start_ms, range_end_ms)?;
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.productivity_report(range)).await
}

#[tauri::command]
pub async fn export_productivity_report_csv(
    range_start_ms: i64,
    range_end_ms: i64,
    app: AppHandle,
    statistics: State<'_, StatisticsService>,
    repository: State<'_, ActivityRepository>,
) -> Result<Option<String>, String> {
    let range = TimeRange::new(range_start_ms, range_end_ms).map_err(|error| error.to_string())?;
    let statistics = statistics.inner().clone();
    let repository = repository.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let report = statistics
            .productivity_report(range)
            .map_err(|error| error.to_string())?;
        let mut csv = "section,label,active_duration_ms,idle_duration_ms,value\n".to_owned();
        csv.push_str(&format!(
            "summary,total,{},{},\n",
            report.active_duration_ms, report.idle_duration_ms
        ));
        for day in report.daily_usage {
            csv.push_str(&format!(
                "daily,{},{},{},\n",
                csv_field(&day.date),
                day.active_duration_ms,
                day.idle_duration_ms
            ));
        }
        for hour in report.hourly_usage {
            csv.push_str(&format!(
                "hourly,{:02}:00,{},0,\n",
                hour.hour, hour.active_duration_ms
            ));
        }
        for category in report.category_usage {
            csv.push_str(&format!(
                "category,{},0,0,{}\n",
                csv_field(&category.category),
                category.duration_ms
            ));
        }
        for plan in repository
            .focus_plan_history(range.start_ms, range.end_ms)
            .map_err(|error| error.to_string())?
        {
            let actual_duration_ms = plan
                .ended_at_ms
                .saturating_sub(plan.started_at_ms)
                .saturating_sub(plan.paused_duration_ms)
                .max(0);
            let planned_duration_ms = plan
                .planned_end_at_ms
                .map(|end| end.saturating_sub(plan.started_at_ms).max(0))
                .unwrap_or_default();
            csv.push_str(&format!(
                "focus_plan,{},{},{},{}\n",
                csv_field(&plan.outcome),
                actual_duration_ms,
                plan.paused_duration_ms.max(0),
                planned_duration_ms
            ));
        }
        let destination = app
            .dialog()
            .file()
            .set_file_name("watchhouse-report.csv")
            .add_filter("CSV", &["csv"])
            .blocking_save_file();
        let Some(path) = destination.and_then(|path| path.into_path().ok()) else {
            return Ok(None);
        };
        fs::write(&path, csv).map_err(|error| error.to_string())?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|error| error.to_string())?
}

fn csv_field(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[tauri::command]
pub async fn get_application_daily_usage(
    application_id: i64,
    range_start_ms: i64,
    range_end_ms: i64,
    statistics: State<'_, StatisticsService>,
) -> IpcResult<Vec<DailyUsage>> {
    let range = TimeRange::new(range_start_ms, range_end_ms)?;
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.application_daily_usage(application_id, range)).await
}

fn parse_date(value: &str) -> IpcResult<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").map_err(|_| {
        AppError::InvalidTimeRange("date must use YYYY-MM-DD format".to_owned()).into()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_iso_calendar_date() {
        assert_eq!(
            parse_date("2026-07-30").expect("date should parse"),
            NaiveDate::from_ymd_opt(2026, 7, 30).expect("date should exist")
        );
    }

    #[test]
    fn rejects_ambiguous_display_date() {
        let error = parse_date("07/30/2026").expect_err("date should fail");
        assert_eq!(error.code, "INVALID_TIME_RANGE");
    }

    #[test]
    fn csv_fields_escape_quotes() {
        assert_eq!(csv_field("Work \"deep\""), "\"Work \"\"deep\"\"\"");
    }
}
