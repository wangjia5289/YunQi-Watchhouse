use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::Manager;
use tauri::{AppHandle, State};
use tauri_plugin_autostart::{AutoLaunchManager, ManagerExt};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::TrackingTrayMenuItem;
use crate::activity::{MonitorHandle, SessionManagerHandle};
use crate::database::{ActivityRepository, Settings};

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ActivityExport<'a> {
    schema_version: u32,
    exported_at_ms: i64,
    records: &'a [crate::database::ActivityRecord],
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsSummary {
    application_version: String,
    database_path: String,
    database_bytes: u64,
    wal_bytes: u64,
    icon_cache_bytes: u64,
    log_bytes: u64,
    application_count: i64,
    session_count: i64,
}

#[tauri::command]
pub fn get_settings(repository: State<'_, ActivityRepository>) -> Result<Settings, String> {
    repository.settings().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn complete_onboarding(repository: State<'_, ActivityRepository>) -> Result<Settings, String> {
    repository
        .complete_onboarding(now_ms()?)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_settings(
    settings: Settings,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
    monitor: State<'_, MonitorHandle>,
) -> Result<Settings, String> {
    let previous = repository.settings().map_err(|error| error.to_string())?;
    validate_settings(&settings)?;
    let autostart = app.autolaunch();
    set_autostart(&autostart, settings.launch_at_login)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis() as i64;
    let saved = match repository.update_settings(&settings, now) {
        Ok(saved) => saved,
        Err(error) => {
            let _ = set_autostart(&autostart, previous.launch_at_login);
            return Err(error.to_string());
        }
    };
    if let Err(error) = monitor.set_idle_threshold(std::time::Duration::from_secs(
        saved.idle_threshold_seconds as u64,
    )) {
        let _ = repository.update_settings(&previous, now);
        let _ = set_autostart(&autostart, previous.launch_at_login);
        return Err(error.to_string());
    }
    Ok(saved)
}

fn validate_settings(settings: &Settings) -> Result<(), String> {
    if !(30..=3600).contains(&settings.idle_threshold_seconds) {
        return Err("idle threshold must be between 30 and 3600 seconds".to_owned());
    }
    if !matches!(settings.appearance.as_str(), "SYSTEM" | "LIGHT" | "DARK") {
        return Err("appearance must be SYSTEM, LIGHT, or DARK".to_owned());
    }
    Ok(())
}

fn set_autostart(manager: &AutoLaunchManager, enabled: bool) -> Result<(), String> {
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn delete_all_activity(
    repository: State<'_, ActivityRepository>,
    monitor: State<'_, MonitorHandle>,
    session_manager: State<'_, SessionManagerHandle>,
) -> Result<(), String> {
    let was_paused = monitor.is_paused();
    monitor.set_paused(true);
    let acknowledgement = session_manager.request_pause();
    let repository = repository.inner().clone();
    if let Some(receiver) = acknowledgement {
        let _ = receiver.await;
    }
    let result = repository
        .delete_all_activity()
        .map_err(|error| error.to_string());
    if !was_paused {
        monitor.set_paused(false);
    }
    result
}

#[tauri::command]
pub async fn export_activity(
    format: String,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> Result<Option<String>, String> {
    let repository = repository.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let records = repository
            .all_activity_records()
            .map_err(|error| error.to_string())?;
        let format = format.to_ascii_lowercase();
        let (extension, contents) = match format.as_str() {
            "json" => (
                "json",
                serde_json::to_string_pretty(&ActivityExport {
                    schema_version: 1,
                    exported_at_ms: now_ms()?,
                    records: &records,
                })
                .map_err(|error| error.to_string())?,
            ),
            "csv" => ("csv", records_to_csv(&records)),
            _ => return Err("export format must be json or csv".to_owned()),
        };
        let destination = app
            .dialog()
            .file()
            .set_file_name(format!("watchhouse-activity.{extension}"))
            .add_filter(extension.to_ascii_uppercase(), &[extension])
            .blocking_save_file();
        let Some(path) = destination.and_then(|path| path.into_path().ok()) else {
            return Ok(None);
        };
        fs::write(&path, contents).map_err(|error| error.to_string())?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|error| error.to_string())?
}

fn records_to_csv(records: &[crate::database::ActivityRecord]) -> String {
    let mut output =
        "# watchhouse_export_schema=1\nsession_id,state,application_name,bundle_identifier,started_at_ms,ended_at_ms,duration_ms,closed_reason\n"
            .to_owned();
    for record in records {
        let application_name = record
            .application
            .as_ref()
            .map(|app| app.name.as_str())
            .unwrap_or("");
        let bundle = record
            .application
            .as_ref()
            .and_then(|app| app.bundle_id.as_deref())
            .unwrap_or("");
        let reason = record
            .session
            .closed_reason
            .map(|reason| format!("{reason:?}"))
            .unwrap_or_default();
        output.push_str(&format!(
            "{},{:?},{},{},{},{},{},{}\n",
            record.session.id,
            record.session.state,
            csv_field(application_name),
            csv_field(bundle),
            record.session.started_at_ms,
            record.session.ended_at_ms,
            record.session.duration_ms,
            reason
        ));
    }
    output
}

fn now_ms() -> Result<i64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis() as i64)
}

fn csv_field(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

#[tauri::command]
pub fn open_data_directory(app: AppHandle) -> Result<(), String> {
    let path = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    app.opener()
        .open_path(path.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn open_log_directory(app: AppHandle) -> Result<(), String> {
    let path = app
        .path()
        .app_log_dir()
        .map_err(|error| error.to_string())?;
    app.opener()
        .open_path(path.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_diagnostics_summary(
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> Result<DiagnosticsSummary, String> {
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    let database = app_data.join("watchhouse.sqlite3");
    let wal = app_data.join("watchhouse.sqlite3-wal");
    let icons = app_data.join("icons");
    let logs = app
        .path()
        .app_log_dir()
        .map_err(|error| error.to_string())?;
    let (application_count, session_count) = repository
        .record_counts()
        .map_err(|error| error.to_string())?;
    Ok(DiagnosticsSummary {
        application_version: app.package_info().version.to_string(),
        database_path: database.to_string_lossy().into_owned(),
        database_bytes: file_size(&database),
        wal_bytes: file_size(&wal),
        icon_cache_bytes: directory_size(&icons),
        log_bytes: directory_size(&logs),
        application_count,
        session_count,
    })
}

fn file_size(path: &std::path::Path) -> u64 {
    fs::metadata(path)
        .map(|metadata| metadata.len())
        .unwrap_or(0)
}

fn directory_size(path: &std::path::Path) -> u64 {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                directory_size(&path)
            } else {
                file_size(&path)
            }
        })
        .sum()
}

#[tauri::command]
pub async fn backup_database(
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> Result<Option<String>, String> {
    let repository = repository.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let destination = app
            .dialog()
            .file()
            .set_file_name("watchhouse-backup.sqlite3")
            .add_filter("SQLite Database", &["sqlite3"])
            .blocking_save_file();
        let Some(path) = destination.and_then(|path| path.into_path().ok()) else {
            return Ok(None);
        };
        repository
            .backup_database(&path)
            .map_err(|error| error.to_string())?;
        log::info!("database backup completed");
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn restore_database(app: AppHandle) -> Result<bool, String> {
    let dialog_app = app.clone();
    let path = tauri::async_runtime::spawn_blocking(move || {
        dialog_app
            .dialog()
            .file()
            .add_filter("SQLite Database", &["sqlite3", "db"])
            .blocking_pick_file()
            .and_then(|path| path.into_path().ok())
    })
    .await
    .map_err(|error| error.to_string())?;
    let Some(path) = path else {
        return Ok(false);
    };

    let monitor = app.state::<MonitorHandle>();
    let session_manager = app.state::<SessionManagerHandle>();
    let repository = app.state::<ActivityRepository>().inner().clone();
    monitor.set_paused(true);
    let acknowledgement = session_manager.request_pause();
    if let Some(receiver) = acknowledgement {
        let _ = receiver.await;
    }
    let restore_repository = repository.clone();
    tauri::async_runtime::spawn_blocking(move || restore_repository.restore_database(&path))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let restored_settings = repository.settings().map_err(|error| error.to_string())?;
    monitor
        .set_idle_threshold(std::time::Duration::from_secs(
            restored_settings.idle_threshold_seconds as u64,
        ))
        .map_err(|error| error.to_string())?;
    let _ = app
        .state::<TrackingTrayMenuItem>()
        .0
        .set_text("Resume Tracking");
    log::info!("database restore completed; tracking remains paused");
    Ok(true)
}

#[tauri::command]
pub async fn optimize_database(app: AppHandle) -> Result<(), String> {
    let monitor = app.state::<MonitorHandle>();
    let was_paused = monitor.is_paused();
    monitor.set_paused(true);
    let acknowledgement = app.state::<SessionManagerHandle>().request_pause();
    let repository = app.state::<ActivityRepository>().inner().clone();
    if let Some(receiver) = acknowledgement {
        let _ = receiver.await;
    }
    let result = tauri::async_runtime::spawn_blocking(move || repository.optimize_database())
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result.map_err(|error| error.to_string()));

    if !was_paused {
        app.state::<MonitorHandle>().set_paused(false);
        let _ = app
            .state::<TrackingTrayMenuItem>()
            .0
            .set_text("Pause Tracking");
    }
    result?;
    log::info!("database optimization completed");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_fields_escape_quotes() {
        assert_eq!(csv_field("A \"quoted\" app"), "\"A \"\"quoted\"\" app\"");
    }

    #[test]
    fn csv_export_declares_its_schema() {
        assert!(records_to_csv(&[]).starts_with("# watchhouse_export_schema=1\n"));
    }

    #[test]
    fn settings_validation_rejects_values_before_side_effects() {
        let invalid_threshold = Settings {
            idle_threshold_seconds: 10,
            launch_at_login: false,
            start_tracking_automatically: true,
            hide_to_tray_on_close: true,
            record_window_titles: false,
            appearance: "SYSTEM".to_owned(),
            onboarding_completed: true,
        };
        assert!(validate_settings(&invalid_threshold).is_err());

        let invalid_appearance = Settings {
            idle_threshold_seconds: 180,
            appearance: "SEPIA".to_owned(),
            ..invalid_threshold
        };
        assert!(validate_settings(&invalid_appearance).is_err());
    }
}
