use std::time::{SystemTime, UNIX_EPOCH};

use tauri::State;

use crate::{
    database::{ActivityRepository, UsageLimitRule, UsageLimitRuleInput, UsageLimitTargets},
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

fn now_ms() -> Result<i64, AppError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .map_err(|_| AppError::InvalidSystemClock)
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
}
