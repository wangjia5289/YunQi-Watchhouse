pub(crate) mod activity;
pub(crate) mod applications;
#[path = "../backup_crypto.rs"]
pub(crate) mod backup_crypto;
pub(crate) mod category_rules;
pub(crate) mod encrypted_backup;
pub(crate) mod focus;
pub(crate) mod settings;
pub(crate) mod statistics;
pub(crate) mod timeline;
pub(crate) mod updater;
pub(crate) mod usage_limits;
pub(crate) mod weekly_reports;

pub use activity::{get_current_activity, set_tracking_paused};
pub use applications::{
    clear_application_icon_cache, get_application_icon, update_application_preferences,
};
pub use category_rules::{
    create_category_rule, delete_category_rule, get_category_rules, preview_category_rule,
    reapply_category_rules, update_category_rule,
};
pub use encrypted_backup::{create_encrypted_database_backup, restore_encrypted_database_backup};
pub use focus::{get_focus_mode, set_focus_mode};
pub use settings::{
    backup_database, choose_backup_directory, complete_onboarding, create_automatic_backup_now,
    delete_all_activity, export_activity, get_diagnostics_summary, get_maintenance_preview,
    get_maintenance_status, get_settings, open_backup_directory, open_data_directory,
    open_log_directory, optimize_database, restore_database, run_data_maintenance,
    run_diagnostics_repair, update_settings,
};
pub use statistics::{
    get_app_usage, get_application_daily_usage, get_category_usage, get_daily_usage, get_timeline,
    get_today_focus_summary, get_today_summary,
};
pub use timeline::{
    delete_timeline_session, delete_timeline_sessions, import_activity, merge_timeline_sessions,
    preview_activity_import, undo_timeline_edit, update_timeline_session,
    update_timeline_session_categories, update_timeline_session_notes,
};
pub use updater::{check_for_updates, install_update};
pub use usage_limits::{
    add_temporary_usage_limit_minutes, clear_temporary_usage_limit_minutes, create_usage_limit,
    delete_usage_limit, get_today_usage_limit_progress, get_usage_limit_reminder_history,
    get_usage_limit_targets, get_usage_limits, silence_usage_limit_notifications_for_today,
    snooze_usage_limit_notifications, update_usage_limit,
};
pub use weekly_reports::{
    archive_weekly_report, delete_weekly_report_archive, get_weekly_report_archives,
    send_weekly_report_notification,
};

use serde::Serialize;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcError {
    pub code: &'static str,
    pub message: String,
}

impl From<AppError> for IpcError {
    fn from(error: AppError) -> Self {
        let code = match error {
            AppError::InvalidTimeRange(_) => "INVALID_TIME_RANGE",
            AppError::Database(_) | AppError::Migration(_) => "DATABASE_ERROR",
            _ => "INTERNAL_ERROR",
        };
        Self {
            code,
            message: error.to_string(),
        }
    }
}

pub type IpcResult<T> = Result<T, IpcError>;

async fn run_blocking<T, F>(operation: F) -> IpcResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, AppError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(operation)
        .await
        .map_err(|error| IpcError {
            code: "TASK_ERROR",
            message: error.to_string(),
        })?
        .map_err(Into::into)
}
