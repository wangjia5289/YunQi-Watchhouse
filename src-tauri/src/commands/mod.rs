pub(crate) mod activity;
pub(crate) mod applications;
pub(crate) mod focus;
pub(crate) mod settings;
pub(crate) mod statistics;
pub(crate) mod timeline;

pub use activity::{get_current_activity, set_tracking_paused};
pub use applications::{
    clear_application_icon_cache, get_application_icon, update_application_preferences,
};
pub use focus::{get_focus_mode, set_focus_mode};
pub use settings::{
    backup_database, choose_backup_directory, complete_onboarding, create_automatic_backup_now,
    delete_all_activity, export_activity, get_diagnostics_summary, get_maintenance_preview,
    get_maintenance_status, get_settings, open_backup_directory, open_data_directory,
    open_log_directory, optimize_database, restore_database, run_data_maintenance, update_settings,
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
