use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Duration, Local, NaiveDate, TimeZone};
use tauri::State;

use crate::{
    database::{
        ActivityRepository, UsageLimitDailyException, UsageLimitReminderHistoryEntry,
        UsageLimitRule, UsageLimitRuleInput, UsageLimitTargets,
    },
    error::AppError,
    statistics::{StatisticsService, UsageLimitProgress},
};

use super::{IpcResult, run_blocking};

#[tauri::command]
pub async fn get_usage_limits(
    repository: State<'_, ActivityRepository>,
) -> IpcResult<Vec<UsageLimitRule>> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.usage_limit_rules()).await
}

#[tauri::command]
pub async fn get_usage_limit_targets(
    repository: State<'_, ActivityRepository>,
) -> IpcResult<UsageLimitTargets> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.usage_limit_targets()).await
}

#[tauri::command]
pub async fn create_usage_limit(
    rule: UsageLimitRuleInput,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<UsageLimitRule> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.create_usage_limit(&rule, now_ms()?)).await
}

#[tauri::command]
pub async fn update_usage_limit(
    rule_id: i64,
    rule: UsageLimitRuleInput,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<UsageLimitRule> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.update_usage_limit(rule_id, &rule, now_ms()?)).await
}

#[tauri::command]
pub async fn delete_usage_limit(
    rule_id: i64,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<()> {
    let repository = repository.inner().clone();
    run_blocking(move || repository.delete_usage_limit(rule_id)).await
}

#[tauri::command]
pub async fn get_today_usage_limit_progress(
    statistics: State<'_, StatisticsService>,
) -> IpcResult<Vec<UsageLimitProgress>> {
    let statistics = statistics.inner().clone();
    run_blocking(move || statistics.today_usage_limit_progress()).await
}

#[tauri::command]
pub async fn get_usage_limit_reminder_history(
    days: Option<i64>,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<Vec<UsageLimitReminderHistoryEntry>> {
    let repository = repository.inner().clone();
    run_blocking(move || {
        let end_date = Local::now().date_naive();
        let (start_date, end_date) =
            usage_limit_reminder_history_range(end_date, days.unwrap_or(7))?;
        repository.usage_limit_reminder_history(&start_date.to_string(), &end_date.to_string())
    })
    .await
}

#[tauri::command]
pub async fn snooze_usage_limit_notifications(
    rule_id: i64,
    minutes: i64,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<UsageLimitDailyException> {
    let repository = repository.inner().clone();
    run_blocking(move || {
        let (local_date, now_ms, day_start_ms, day_end_ms) = today_usage_limit_context()?;
        repository.snooze_usage_limit_notifications(
            rule_id,
            &local_date,
            minutes,
            now_ms,
            day_start_ms,
            day_end_ms,
        )
    })
    .await
}

#[tauri::command]
pub async fn silence_usage_limit_notifications_for_today(
    rule_id: i64,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<UsageLimitDailyException> {
    let repository = repository.inner().clone();
    run_blocking(move || {
        let (local_date, now_ms, _, _) = today_usage_limit_context()?;
        repository.silence_usage_limit_notifications_for_today(rule_id, &local_date, now_ms)
    })
    .await
}

#[tauri::command]
pub async fn add_temporary_usage_limit_minutes(
    rule_id: i64,
    minutes: i64,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<UsageLimitDailyException> {
    let repository = repository.inner().clone();
    run_blocking(move || {
        let (local_date, now_ms, _, _) = today_usage_limit_context()?;
        repository.add_temporary_usage_limit_minutes(rule_id, &local_date, minutes, now_ms)
    })
    .await
}

#[tauri::command]
pub async fn clear_temporary_usage_limit_minutes(
    rule_id: i64,
    repository: State<'_, ActivityRepository>,
) -> IpcResult<UsageLimitDailyException> {
    let repository = repository.inner().clone();
    run_blocking(move || {
        let (local_date, now_ms, _, _) = today_usage_limit_context()?;
        repository.clear_temporary_usage_limit_minutes(rule_id, &local_date, now_ms)
    })
    .await
}

fn now_ms() -> Result<i64, AppError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .map_err(|_| AppError::InvalidSystemClock)
}

fn today_usage_limit_context() -> Result<(String, i64, i64, i64), AppError> {
    let now_ms = now_ms()?;
    let local = Local.timestamp_millis_opt(now_ms).single().ok_or_else(|| {
        AppError::InvalidTimeRange("current local time could not be resolved".to_owned())
    })?;
    let range = crate::statistics::local_day_range(local.date_naive())?;
    Ok((
        local.date_naive().to_string(),
        now_ms,
        range.start_ms,
        range.end_ms,
    ))
}

fn usage_limit_reminder_history_range(
    end_date: NaiveDate,
    days: i64,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    if !(1..=90).contains(&days) {
        return Err(AppError::InvalidTimeRange(
            "usage limit reminder history days must be between 1 and 90".to_owned(),
        ));
    }
    let start_date = end_date
        .checked_sub_signed(Duration::days(days - 1))
        .ok_or_else(|| {
            AppError::InvalidTimeRange("usage limit reminder history date overflow".to_owned())
        })?;
    Ok((start_date, end_date))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_limit_input_uses_the_documented_enum_and_camel_case_fields() {
        let input: UsageLimitRuleInput = serde_json::from_value(serde_json::json!({
            "scopeType": "APPLICATION",
            "applicationId": 42,
            "category": null,
            "weekdayLimitMinutes": 120,
            "weekendLimitMinutes": 180,
            "notificationsEnabled": true,
            "enabled": true
        }))
        .expect("documented input should deserialize");
        assert_eq!(input.application_id, Some(42));
        assert!(input.notifications_enabled);
    }

    #[test]
    fn reminder_history_days_are_bounded_and_inclusive() {
        let end_date = NaiveDate::from_ymd_opt(2026, 7, 31).expect("date should be valid");
        assert_eq!(
            usage_limit_reminder_history_range(end_date, 7).expect("seven days should work"),
            (
                NaiveDate::from_ymd_opt(2026, 7, 25).expect("date should be valid"),
                end_date
            )
        );
        assert!(usage_limit_reminder_history_range(end_date, 0).is_err());
        assert!(usage_limit_reminder_history_range(end_date, 91).is_err());
    }
}
