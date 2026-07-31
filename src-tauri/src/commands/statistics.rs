use chrono::NaiveDate;
use tauri::State;

use crate::{
    error::AppError,
    statistics::{
        AppUsage, CategoryUsage, DailyUsage, FocusSummary, StatisticsService, TimeRange,
        TimelineEntry, TodaySummary,
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
}
