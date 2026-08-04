use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::{AutoLaunchManager, ManagerExt};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::GlobalShortcutExt;
use tauri_plugin_notification::{NotificationExt, PermissionState};
use tauri_plugin_opener::OpenerExt;

use crate::activity::{MonitorHandle, SessionManagerHandle};
use crate::commands::backup_credentials;
use crate::database::{ActivityRepository, Settings, ShortcutSettings};
use crate::database::{
    DataHealthRepairResult, DataHealthSummary, DataHealthUndoStatus, MaintenancePreview,
    MaintenanceResult,
};
use crate::maintenance::{MaintenanceStatus, MaintenanceStatusState};
use crate::{
    AppLocaleState, DatabaseMaintenanceState, FocusCountdownTrayMenuItem,
    FocusTemplateTrayMenuItems, FocusTrayMenuItem, QuitTrayMenuItem, ShortcutSettingsState,
    ShowTrayMenuItem, TrackingTrayMenuItem, parse_shortcut,
};

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
    automatic_backup_bytes: u64,
    automatic_backup_count: usize,
    application_count: i64,
    session_count: i64,
    database_integrity_ok: bool,
    accessibility_permission: crate::platform::AccessibilityPermission,
    notification_permission: NotificationPermission,
    tracking_paused: bool,
    automatic_backup_enabled: bool,
    last_backup_at_ms: i64,
    backup_directory_available: bool,
    log_directory_available: bool,
    maintenance_last_error: Option<String>,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsRepairResult {
    backup_path: String,
    trimmed_session_count: usize,
    deleted_session_count: usize,
    icon_cache_cleared: bool,
    database_optimized: bool,
}

#[tauri::command]
pub fn get_settings(repository: State<'_, ActivityRepository>) -> Result<Settings, String> {
    repository.settings().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_shortcut_settings(
    repository: State<'_, ActivityRepository>,
) -> Result<ShortcutSettings, String> {
    repository
        .shortcut_settings()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_shortcut_settings(
    settings: ShortcutSettings,
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> Result<ShortcutSettings, String> {
    let shortcuts = [
        parse_shortcut(&settings.toggle_focus)?,
        parse_shortcut(&settings.pause_focus)?,
        parse_shortcut(&settings.start_template)?,
    ];
    let ids = shortcuts
        .iter()
        .flatten()
        .map(|shortcut| shortcut.id())
        .collect::<std::collections::HashSet<_>>();
    if ids.len() != shortcuts.iter().flatten().count() {
        return Err("each enabled action must use a different shortcut".to_owned());
    }

    let previous = app.state::<ShortcutSettingsState>().snapshot();
    app.global_shortcut()
        .unregister_all()
        .map_err(|error| error.to_string())?;
    for shortcut in shortcuts.into_iter().flatten() {
        if let Err(error) = app.global_shortcut().register(shortcut) {
            let _ = app.global_shortcut().unregister_all();
            for old in [
                parse_shortcut(&previous.toggle_focus)?,
                parse_shortcut(&previous.pause_focus)?,
                parse_shortcut(&previous.start_template)?,
            ]
            .into_iter()
            .flatten()
            {
                let _ = app.global_shortcut().register(old);
            }
            return Err(format!("shortcut is unavailable: {error}"));
        }
    }
    match repository.update_shortcut_settings(&settings, now_ms()?) {
        Ok(saved) => {
            app.state::<ShortcutSettingsState>().replace(saved.clone());
            Ok(saved)
        }
        Err(error) => {
            let _ = app.global_shortcut().unregister_all();
            for old in [
                parse_shortcut(&previous.toggle_focus)?,
                parse_shortcut(&previous.pause_focus)?,
                parse_shortcut(&previous.start_template)?,
            ]
            .into_iter()
            .flatten()
            {
                let _ = app.global_shortcut().register(old);
            }
            Err(error.to_string())
        }
    }
}

#[tauri::command]
pub fn set_app_locale(locale: String, app: AppHandle) -> Result<(), String> {
    if !matches!(locale.as_str(), "en" | "zh-CN") {
        return Err("locale must be en or zh-CN".to_owned());
    }
    let chinese = locale == "zh-CN";
    app.state::<AppLocaleState>().set_chinese(chinese);
    let paused = app.state::<MonitorHandle>().is_paused();
    let focus = app.state::<crate::focus::FocusModeState>().snapshot();
    app.state::<ShowTrayMenuItem>()
        .0
        .set_text(if chinese {
            "显示 Watchhouse"
        } else {
            "Show Watchhouse"
        })
        .map_err(|error| error.to_string())?;
    app.state::<TrackingTrayMenuItem>()
        .0
        .set_text(match (chinese, paused) {
            (true, true) => "继续追踪",
            (true, false) => "暂停追踪",
            (false, true) => "Resume Tracking",
            (false, false) => "Pause Tracking",
        })
        .map_err(|error| error.to_string())?;
    app.state::<FocusTrayMenuItem>()
        .0
        .set_text(match (chinese, focus.active) {
            (true, true) => "结束专注模式",
            (true, false) => "开始专注模式",
            (false, true) => "End Focus Mode",
            (false, false) => "Start Focus Mode",
        })
        .map_err(|error| error.to_string())?;
    if !focus.active {
        app.state::<FocusCountdownTrayMenuItem>()
            .0
            .set_text(if chinese {
                "没有进行中的专注计划"
            } else {
                "No active focus plan"
            })
            .map_err(|error| error.to_string())?;
    }
    app.state::<QuitTrayMenuItem>()
        .0
        .set_text(if chinese { "退出" } else { "Quit" })
        .map_err(|error| error.to_string())?;
    let templates = app
        .state::<ActivityRepository>()
        .focus_plan_templates()
        .map_err(|error| error.to_string())?;
    for (item, template) in app
        .state::<FocusTemplateTrayMenuItems>()
        .0
        .iter()
        .zip(templates.iter())
    {
        item.set_text(if chinese {
            format!(
                "开始：{}（{} 分钟）",
                template.name, template.duration_minutes
            )
        } else {
            format!(
                "Start: {} ({} min)",
                template.name, template.duration_minutes
            )
        })
        .map_err(|error| error.to_string())?;
    }
    crate::update_tray_overview(&app);
    Ok(())
}

#[tauri::command]
pub fn get_data_health_summary(
    repository: State<'_, ActivityRepository>,
) -> Result<DataHealthSummary, String> {
    repository
        .data_health_summary()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn repair_data_health(
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> Result<DataHealthRepairResult, String> {
    let repository = repository.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?
            .join("backups");
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let path = directory.join(format!("before-health-repair-{}.sqlite3", now_ms()?));
        repository
            .backup_database(&path)
            .map_err(|error| error.to_string())?;
        repository
            .repair_data_health(&path.to_string_lossy())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn get_data_health_undo_status(
    repository: State<'_, ActivityRepository>,
) -> Result<DataHealthUndoStatus, String> {
    repository
        .data_health_undo_status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn undo_data_health_repair(repository: State<'_, ActivityRepository>) -> Result<usize, String> {
    repository
        .undo_data_health_repair()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_accessibility_permission() -> crate::platform::AccessibilityPermission {
    crate::platform::accessibility_permission()
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum NotificationPermission {
    Granted,
    Denied,
    Prompt,
}

impl From<PermissionState> for NotificationPermission {
    fn from(value: PermissionState) -> Self {
        match value {
            PermissionState::Granted => Self::Granted,
            PermissionState::Denied => Self::Denied,
            PermissionState::Prompt | PermissionState::PromptWithRationale => Self::Prompt,
        }
    }
}

#[tauri::command]
pub fn get_notification_permission(app: AppHandle) -> Result<NotificationPermission, String> {
    app.notification()
        .permission_state()
        .map(NotificationPermission::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn request_notification_permission(app: AppHandle) -> Result<NotificationPermission, String> {
    app.notification()
        .request_permission()
        .map(NotificationPermission::from)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn send_test_notification(app: AppHandle) -> Result<(), String> {
    let permission = app
        .notification()
        .permission_state()
        .map_err(|error| error.to_string())?;
    if permission != PermissionState::Granted {
        return Err("notification permission has not been granted".to_owned());
    }
    app.notification()
        .builder()
        .title("Watchhouse notifications are ready")
        .body("Break reminders and focus plan alerts can appear on this Mac.")
        .show()
        .map_err(|error| error.to_string())
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
    validate_automatic_encrypted_backup_credentials(
        settings.automatic_encrypted_backup_enabled && !previous.automatic_encrypted_backup_enabled,
        || backup_credentials::load().map(|password| password.is_some()),
    )?;
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
    if !matches!(settings.retention_days, 0 | 30 | 90 | 180 | 365) {
        return Err("retention days must be 0, 30, 90, 180, or 365".to_owned());
    }
    if !matches!(settings.backup_interval.as_str(), "DAILY" | "WEEKLY") {
        return Err("backup interval must be DAILY or WEEKLY".to_owned());
    }
    if !(1..=20).contains(&settings.backup_keep_count) {
        return Err("backup keep count must be between 1 and 20".to_owned());
    }
    if !(0..=1440).contains(&settings.daily_focus_goal_minutes) {
        return Err("daily focus goal must be between 0 and 1440 minutes".to_owned());
    }
    if !(1..=60).contains(&settings.focus_block_gap_minutes) {
        return Err("focus block gap must be between 1 and 60 minutes".to_owned());
    }
    if !matches!(settings.break_reminder_minutes, 30 | 45 | 60 | 90 | 120) {
        return Err("break reminder must be 30, 45, 60, 90, or 120 minutes".to_owned());
    }
    if !(1..=7).contains(&settings.weekly_report_notification_weekday) {
        return Err("weekly report notification weekday must be between 1 and 7".to_owned());
    }
    for value in [
        &settings.quiet_hours_start,
        &settings.quiet_hours_end,
        &settings.weekly_report_notification_time,
    ] {
        let valid = value.len() == 5
            && value.as_bytes().get(2) == Some(&b':')
            && value[..2].parse::<u8>().is_ok_and(|hour| hour < 24)
            && value[3..].parse::<u8>().is_ok_and(|minute| minute < 60);
        if !valid {
            return Err("quiet hours must use HH:MM in 24-hour time".to_owned());
        }
    }
    Ok(())
}

fn validate_automatic_encrypted_backup_credentials(
    enabled: bool,
    load_password: impl FnOnce() -> Result<bool, String>,
) -> Result<(), String> {
    if !enabled {
        return Ok(());
    }
    if load_password()? {
        Ok(())
    } else {
        Err("Automatic encrypted backup password is not configured.".to_owned())
    }
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
        "# watchhouse_export_schema=1\nsession_id,state,application_name,bundle_identifier,started_at_ms,ended_at_ms,duration_ms,closed_reason,window_title,note\n"
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
            "{},{:?},{},{},{},{},{},{},{},{}\n",
            record.session.id,
            record.session.state,
            csv_field(application_name),
            csv_field(bundle),
            record.session.started_at_ms,
            record.session.ended_at_ms,
            record.session.duration_ms,
            reason,
            csv_field(record.session.window_title.as_deref().unwrap_or("")),
            csv_field(record.session.note.as_deref().unwrap_or(""))
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
    format!(
        "\"{}\"",
        value.replace(['\r', '\n'], " ").replace('"', "\"\"")
    )
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
    let settings = repository.settings().map_err(|error| error.to_string())?;
    let backups = settings
        .backup_directory
        .filter(|path| !path.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| app_data.join("backups"));
    let (application_count, session_count) = repository
        .record_counts()
        .map_err(|error| error.to_string())?;
    let database_integrity_ok = repository
        .database_integrity_ok()
        .map_err(|error| error.to_string())?;
    let notification_permission = app
        .notification()
        .permission_state()
        .map(NotificationPermission::from)
        .map_err(|error| error.to_string())?;
    let maintenance = app.state::<MaintenanceStatusState>().snapshot();
    Ok(DiagnosticsSummary {
        application_version: app.package_info().version.to_string(),
        database_path: database.to_string_lossy().into_owned(),
        database_bytes: file_size(&database),
        wal_bytes: file_size(&wal),
        icon_cache_bytes: directory_size(&icons),
        log_bytes: directory_size(&logs),
        automatic_backup_bytes: directory_size(&backups),
        automatic_backup_count: file_count(&backups),
        application_count,
        session_count,
        database_integrity_ok,
        accessibility_permission: crate::platform::accessibility_permission(),
        notification_permission,
        tracking_paused: app.state::<MonitorHandle>().is_paused(),
        automatic_backup_enabled: settings.automatic_backup_enabled,
        last_backup_at_ms: settings.last_backup_at_ms,
        backup_directory_available: directory_available(&backups),
        log_directory_available: directory_available(&logs),
        maintenance_last_error: maintenance.last_error,
    })
}

#[tauri::command]
pub async fn run_diagnostics_repair(app: AppHandle) -> Result<DiagnosticsRepairResult, String> {
    let _maintenance_guard = app.state::<DatabaseMaintenanceState>().try_begin()?;
    let monitor = app.state::<MonitorHandle>();
    let was_paused = monitor.is_paused();
    monitor.set_paused(true);
    if let Some(receiver) = app.state::<SessionManagerHandle>().request_pause() {
        let _ = receiver.await;
    }

    let operation: Result<DiagnosticsRepairResult, String> = async {
        let repository = app.state::<ActivityRepository>().inner().clone();
        let app_data = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        tauri::async_runtime::spawn_blocking(move || {
            let backup_directory = app_data.join("backups");
            fs::create_dir_all(&backup_directory).map_err(|error| error.to_string())?;
            let backup_path =
                backup_directory.join(format!("before-diagnostics-repair-{}.sqlite3", now_ms()?));
            repository
                .backup_database(&backup_path)
                .map_err(|error| error.to_string())?;
            let health = repository
                .repair_data_health(&backup_path.to_string_lossy())
                .map_err(|error| error.to_string())?;
            let database_optimized = match repository.optimize_database() {
                Ok(()) => true,
                Err(error) => {
                    log::warn!("diagnostics repair could not optimize the database: {error}");
                    false
                }
            };
            let icons = app_data.join("icons");
            let icon_cache_cleared = match (|| -> Result<(), std::io::Error> {
                if icons.exists() {
                    fs::remove_dir_all(&icons)?;
                }
                fs::create_dir_all(&icons)
            })() {
                Ok(()) => true,
                Err(error) => {
                    log::warn!("diagnostics repair could not refresh the icon cache: {error}");
                    false
                }
            };
            Ok(DiagnosticsRepairResult {
                backup_path: backup_path.to_string_lossy().into_owned(),
                trimmed_session_count: health.trimmed_session_count,
                deleted_session_count: health.deleted_session_count,
                icon_cache_cleared,
                database_optimized,
            })
        })
        .await
        .map_err(|error| error.to_string())?
    }
    .await;

    if !was_paused {
        monitor.set_paused(false);
        let _ = app.state::<TrackingTrayMenuItem>().0.set_text(
            if app.state::<AppLocaleState>().is_chinese() {
                "暂停追踪"
            } else {
                "Pause Tracking"
            },
        );
    }
    operation
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

fn file_count(path: &std::path::Path) -> usize {
    let Ok(entries) = fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_file())
        .count()
}

fn directory_available(path: &std::path::Path) -> bool {
    if path.exists() {
        return path.is_dir()
            && fs::metadata(path)
                .map(|metadata| !metadata.permissions().readonly())
                .unwrap_or(false);
    }
    path.parent().is_some_and(|parent| {
        parent.exists()
            && fs::metadata(parent)
                .map(|metadata| !metadata.permissions().readonly())
                .unwrap_or(false)
    })
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
pub async fn choose_backup_directory(app: AppHandle) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        Ok(app
            .dialog()
            .file()
            .blocking_pick_folder()
            .and_then(|path| path.into_path().ok())
            .map(|path| path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub fn open_backup_directory(
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> Result<(), String> {
    let settings = repository.settings().map_err(|error| error.to_string())?;
    let directory = settings
        .backup_directory
        .filter(|path| !path.trim().is_empty())
        .map(std::path::PathBuf::from)
        .unwrap_or(
            app.path()
                .app_data_dir()
                .map_err(|error| error.to_string())?
                .join("backups"),
        );
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    app.opener()
        .open_path(directory.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_maintenance_preview(
    repository: State<'_, ActivityRepository>,
) -> Result<MaintenancePreview, String> {
    let settings = repository.settings().map_err(|error| error.to_string())?;
    repository
        .maintenance_preview(now_ms()?, settings.retention_days)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_maintenance_status(status: State<'_, MaintenanceStatusState>) -> MaintenanceStatus {
    status.snapshot()
}

#[tauri::command]
pub async fn run_data_maintenance(
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> Result<MaintenanceResult, String> {
    let repository = repository.inner().clone();
    let settings = repository.settings().map_err(|error| error.to_string())?;
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::maintenance::run_cleanup(&repository, &app_data, now_ms()?, settings.retention_days)
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn create_automatic_backup_now(
    app: AppHandle,
    repository: State<'_, ActivityRepository>,
) -> Result<String, String> {
    let repository = repository.inner().clone();
    let app_data = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        crate::maintenance::create_automatic_backup(&repository, &app_data, now_ms()?)
            .map(|path| path.to_string_lossy().into_owned())
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

    let _maintenance_guard = app.state::<DatabaseMaintenanceState>().try_begin()?;
    let monitor = app.state::<MonitorHandle>();
    let session_manager = app.state::<SessionManagerHandle>();
    let repository = app.state::<ActivityRepository>().inner().clone();
    let was_paused = monitor.is_paused();
    monitor.set_paused(true);
    let acknowledgement = session_manager.request_pause();
    if let Some(receiver) = acknowledgement {
        let _ = receiver.await;
    }
    let restore_repository = repository.clone();
    let restore_result =
        tauri::async_runtime::spawn_blocking(move || restore_repository.restore_database(&path))
            .await
            .map_err(|error| error.to_string())?;
    if let Err(error) = restore_result {
        if !was_paused {
            monitor.set_paused(false);
            let _ = app.state::<TrackingTrayMenuItem>().0.set_text(
                if app.state::<AppLocaleState>().is_chinese() {
                    "暂停追踪"
                } else {
                    "Pause Tracking"
                },
            );
        }
        return Err(error.to_string());
    }
    reload_runtime_after_restore(&app, &repository)?;
    let _ = app.state::<TrackingTrayMenuItem>().0.set_text(
        if app.state::<AppLocaleState>().is_chinese() {
            "继续追踪"
        } else {
            "Resume Tracking"
        },
    );
    log::info!("database restore completed; tracking remains paused");
    Ok(true)
}

pub(crate) fn reload_runtime_after_restore(
    app: &AppHandle,
    repository: &ActivityRepository,
) -> Result<(), String> {
    let restored_settings = repository.settings().map_err(|error| error.to_string())?;
    let restored_shortcuts = repository
        .shortcut_settings()
        .map_err(|error| error.to_string())?;
    let restored_focus = repository
        .focus_mode_status()
        .map_err(|error| error.to_string())?;

    app.state::<MonitorHandle>()
        .set_idle_threshold(std::time::Duration::from_secs(
            restored_settings.idle_threshold_seconds as u64,
        ))
        .map_err(|error| error.to_string())?;

    let focus_status = crate::focus::FocusModeStatus {
        active: restored_focus.active,
        started_at_ms: restored_focus.started_at_ms,
        planned_end_at_ms: restored_focus.planned_end_at_ms,
        paused: restored_focus.paused,
        paused_at_ms: restored_focus.paused_at_ms,
        total_paused_ms: restored_focus.total_paused_ms,
        template_id: restored_focus.template_id,
    };
    let focus_status = app
        .state::<crate::focus::FocusModeState>()
        .replace(focus_status);
    app.state::<ShortcutSettingsState>()
        .replace(restored_shortcuts.clone());

    app.global_shortcut()
        .unregister_all()
        .map_err(|error| error.to_string())?;
    for shortcut in [
        parse_shortcut(&restored_shortcuts.toggle_focus),
        parse_shortcut(&restored_shortcuts.pause_focus),
        parse_shortcut(&restored_shortcuts.start_template),
    ]
    .into_iter()
    .filter_map(|result| match result {
        Ok(shortcut) => shortcut,
        Err(error) => {
            log::warn!("restored shortcut is invalid: {error}");
            None
        }
    }) {
        if let Err(error) = app.global_shortcut().register(shortcut) {
            log::warn!("restored shortcut could not be registered: {error}");
        }
    }

    let chinese = app.state::<AppLocaleState>().is_chinese();
    let _ = app
        .state::<FocusTrayMenuItem>()
        .0
        .set_text(if focus_status.active && chinese {
            "结束专注模式"
        } else if focus_status.active {
            "End Focus Mode"
        } else if chinese {
            "开始专注模式"
        } else {
            "Start Focus Mode"
        });
    let _ = app.emit("focus-mode-changed", &focus_status);
    crate::update_tray_overview(app);
    Ok(())
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
        let _ = app.state::<TrackingTrayMenuItem>().0.set_text(
            if app.state::<AppLocaleState>().is_chinese() {
                "暂停追踪"
            } else {
                "Pause Tracking"
            },
        );
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
            retention_days: 0,
            automatic_backup_enabled: false,
            backup_interval: "WEEKLY".to_owned(),
            backup_keep_count: 5,
            backup_directory: None,
            last_maintenance_at_ms: 0,
            last_backup_at_ms: 0,
            automatic_encrypted_backup_enabled: false,
            last_encrypted_backup_at_ms: 0,
            weekly_report_auto_archive_enabled: false,
            weekly_report_notification_enabled: false,
            weekly_report_notification_weekday: 1,
            weekly_report_notification_time: "09:00".to_owned(),
            daily_focus_goal_minutes: 240,
            focus_block_gap_minutes: 5,
            break_reminders_enabled: false,
            break_reminder_minutes: 60,
            quiet_hours_start: "22:00".to_owned(),
            quiet_hours_end: "08:00".to_owned(),
        };
        assert!(validate_settings(&invalid_threshold).is_err());

        let invalid_appearance = Settings {
            idle_threshold_seconds: 180,
            appearance: "SEPIA".to_owned(),
            ..invalid_threshold
        };
        assert!(validate_settings(&invalid_appearance).is_err());
    }

    #[test]
    fn automatic_encrypted_backups_require_a_stored_password() {
        assert!(
            validate_automatic_encrypted_backup_credentials(false, || {
                panic!("disabled backups must not access secure storage")
            })
            .is_ok()
        );
        assert_eq!(
            validate_automatic_encrypted_backup_credentials(true, || Ok(false)).unwrap_err(),
            "Automatic encrypted backup password is not configured."
        );
        assert!(validate_automatic_encrypted_backup_credentials(true, || Ok(true)).is_ok());
    }

    #[test]
    fn automatic_encrypted_backup_validation_preserves_secure_storage_errors() {
        assert_eq!(
            validate_automatic_encrypted_backup_credentials(true, || {
                Err("secure storage unavailable".to_owned())
            })
            .unwrap_err(),
            "secure storage unavailable"
        );
    }
}
