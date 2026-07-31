use chrono::NaiveDate;
use std::fs;
use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::{
    database::{ActivityRepository, TimelineSearch},
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
    filters: Option<TimelineSearch>,
    statistics: State<'_, StatisticsService>,
) -> IpcResult<TimelinePage> {
    let date = parse_date(&date)?;
    let filters = filters.unwrap_or_default();
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.timeline_page_for_date_filtered(date, offset, limit, &filters))
        .await
}

#[tauri::command]
pub async fn search_timeline_range(
    start_date: String,
    end_date: String,
    offset: usize,
    limit: usize,
    filters: Option<TimelineSearch>,
    statistics: State<'_, StatisticsService>,
) -> IpcResult<TimelinePage> {
    let (start_date, end_date) = parse_timeline_date_range(&start_date, &end_date)?;
    validate_timeline_page_limit(limit)?;
    let filters = filters.unwrap_or_default();
    let statistics = statistics.inner().clone();
    run_blocking(move || {
        statistics
            .timeline_page_for_date_range_filtered(start_date, end_date, offset, limit, &filters)
    })
    .await
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

fn parse_timeline_date_range(
    start_date: &str,
    end_date: &str,
) -> IpcResult<(NaiveDate, NaiveDate)> {
    let start_date = parse_date(start_date)?;
    let end_date = parse_date(end_date)?;
    if end_date < start_date {
        return Err(AppError::InvalidTimeRange(
            "end date must not be before start date".to_owned(),
        )
        .into());
    }
    let inclusive_days = end_date.signed_duration_since(start_date).num_days() + 1;
    if inclusive_days > 366 {
        return Err(AppError::InvalidTimeRange(
            "timeline search range cannot exceed 366 days".to_owned(),
        )
        .into());
    }
    Ok((start_date, end_date))
}

fn validate_timeline_page_limit(limit: usize) -> IpcResult<()> {
    if !(1..=1_000).contains(&limit) {
        return Err(AppError::InvalidTimeRange(
            "timeline page limit must be between 1 and 1000".to_owned(),
        )
        .into());
    }
    Ok(())
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
    fn accepts_timeline_range_of_up_to_366_inclusive_days() {
        let (start, end) = parse_timeline_date_range("2024-01-01", "2024-12-31")
            .expect("366-day range should be valid");
        assert_eq!(end.signed_duration_since(start).num_days() + 1, 366);
    }

    #[test]
    fn rejects_reversed_or_oversized_timeline_range() {
        for (start, end) in [("2026-07-31", "2026-07-30"), ("2024-01-01", "2025-01-01")] {
            let error =
                parse_timeline_date_range(start, end).expect_err("date range should be rejected");
            assert_eq!(error.code, "INVALID_TIME_RANGE");
        }
    }

    #[test]
    fn rejects_timeline_page_limit_outside_supported_range() {
        for limit in [0, 1_001] {
            let error =
                validate_timeline_page_limit(limit).expect_err("page limit should be rejected");
            assert_eq!(error.code, "INVALID_TIME_RANGE");
        }
        validate_timeline_page_limit(1).expect("minimum page limit should be valid");
        validate_timeline_page_limit(1_000).expect("maximum page limit should be valid");
    }

    #[test]
    fn csv_fields_escape_quotes() {
        assert_eq!(csv_field("Work \"deep\""), "\"Work \"\"deep\"\"\"");
    }
}
