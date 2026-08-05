pub mod activity;
pub mod commands;
pub mod database;
pub mod error;
pub mod focus;
pub mod maintenance;
pub mod platform;
pub mod statistics;

use std::{
    str::FromStr,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tauri::{
    Emitter, Manager, PhysicalPosition, Rect, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_notification::{NotificationExt, PermissionState};
#[cfg(not(debug_assertions))]
use tauri_plugin_updater::UpdaterExt;

pub struct TrackingTrayMenuItem(pub MenuItem<tauri::Wry>);
pub struct FocusTrayMenuItem(pub MenuItem<tauri::Wry>);
pub struct FocusCountdownTrayMenuItem(pub MenuItem<tauri::Wry>);
pub struct ShowTrayMenuItem(pub MenuItem<tauri::Wry>);
pub struct QuitTrayMenuItem(pub MenuItem<tauri::Wry>);
pub struct FocusTemplateTrayMenuItems(pub Vec<MenuItem<tauri::Wry>>);
pub struct TodayActiveTrayMenuItem(pub MenuItem<tauri::Wry>);
pub struct CurrentApplicationTrayMenuItem(pub MenuItem<tauri::Wry>);
pub struct TodayFocusTrayMenuItem(pub MenuItem<tauri::Wry>);
pub struct UsageLimitTrayMenuItem(pub MenuItem<tauri::Wry>);
pub struct AppLocaleState(AtomicBool);
pub struct ShortcutSettingsState(Mutex<database::ShortcutSettings>);
#[derive(Clone, Default)]
pub struct DatabaseMaintenanceState(Arc<AtomicBool>);
pub struct DatabaseMaintenanceGuard(Arc<AtomicBool>);

const TRAY_PANEL_WIDTH: f64 = 360.0;
const TRAY_PANEL_HEIGHT: f64 = 430.0;

impl Default for AppLocaleState {
    fn default() -> Self {
        Self::new()
    }
}

impl AppLocaleState {
    pub fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    pub fn is_chinese(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    pub fn set_chinese(&self, chinese: bool) {
        self.0.store(chinese, Ordering::Relaxed);
    }
}

impl ShortcutSettingsState {
    pub fn new(settings: database::ShortcutSettings) -> Self {
        Self(Mutex::new(settings))
    }

    pub fn snapshot(&self) -> database::ShortcutSettings {
        self.0
            .lock()
            .map(|settings| settings.clone())
            .unwrap_or(database::ShortcutSettings {
                toggle_focus: None,
                pause_focus: None,
                start_template: None,
            })
    }

    pub fn replace(&self, settings: database::ShortcutSettings) {
        if let Ok(mut current) = self.0.lock() {
            *current = settings;
        }
    }
}

impl DatabaseMaintenanceState {
    pub fn is_active(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    pub fn try_begin(&self) -> Result<DatabaseMaintenanceGuard, String> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| DatabaseMaintenanceGuard(self.0.clone()))
            .map_err(|_| "Database maintenance is already in progress.".to_owned())
    }
}

impl Drop for DatabaseMaintenanceGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

pub fn parse_shortcut(value: &Option<String>) -> Result<Option<Shortcut>, String> {
    value
        .as_deref()
        .map(Shortcut::from_str)
        .transpose()
        .map_err(|error| error.to_string())
}

fn send_usage_limit_notifications(
    app: &tauri::AppHandle,
    repository: &database::ActivityRepository,
    now_ms: i64,
) {
    match app.notification().permission_state() {
        Ok(PermissionState::Granted) => {}
        Ok(_) => return,
        Err(error) => {
            log::warn!("could not read notification permission for usage limits: {error}");
            return;
        }
    }
    let settings = match repository.settings() {
        Ok(settings) => settings,
        Err(error) => {
            log::warn!("could not read settings for usage limit notifications: {error}");
            return;
        }
    };
    let local_time = chrono::Local::now().format("%H:%M").to_string();
    if focus::is_quiet_hours(
        &local_time,
        &settings.quiet_hours_start,
        &settings.quiet_hours_end,
    ) {
        return;
    }
    let progress = match statistics::StatisticsService::new(repository.clone())
        .today_notifiable_usage_limit_progress()
    {
        Ok(progress) => progress,
        Err(error) => {
            log::warn!("could not calculate usage limit progress: {error}");
            return;
        }
    };
    for item in progress {
        let threshold = match item.threshold_state {
            statistics::UsageLimitThresholdState::Below80 => continue,
            statistics::UsageLimitThresholdState::Reached80 => 80,
            statistics::UsageLimitThresholdState::Reached100 => 100,
        };
        let delivered = match repository.delivered_usage_limit_thresholds(item.id, &item.local_date)
        {
            Ok(delivered) => delivered,
            Err(error) => {
                log::warn!("could not read usage limit alert history: {error}");
                continue;
            }
        };
        if delivered.contains(&threshold) {
            continue;
        }
        let target = item
            .application_name
            .as_deref()
            .or(item.category.as_deref())
            .unwrap_or("Watchhouse");
        let chinese = app.state::<AppLocaleState>().is_chinese();
        let (title, body) = usage_limit_notification_copy(
            target,
            item.used_duration_ms,
            item.limit_minutes,
            threshold,
            chinese,
        );
        if let Err(error) = app.notification().builder().title(title).body(body).show() {
            log::warn!("could not show usage limit notification: {error}");
            continue;
        }
        if let Err(error) = repository.mark_usage_limit_alerts_delivered(
            item.id,
            &item.local_date,
            &[threshold],
            now_ms,
        ) {
            log::warn!("could not persist usage limit alert delivery: {error}");
        }
    }
}

fn usage_limit_notification_copy(
    target: &str,
    used_duration_ms: i64,
    limit_minutes: i64,
    threshold: i64,
    chinese: bool,
) -> (&'static str, String) {
    let used_minutes = used_duration_ms.saturating_add(59_999) / 60_000;
    match (chinese, threshold) {
        (true, 100) => (
            "已达到使用限额",
            format!("{target} 今日已使用 {used_minutes} 分钟，达到每日限额 {limit_minutes} 分钟。"),
        ),
        (true, _) => (
            "接近使用限额",
            format!("{target} 今日已使用 {used_minutes} 分钟，接近每日限额 {limit_minutes} 分钟。"),
        ),
        (false, 100) => (
            "Usage limit reached",
            format!(
                "{target} has been used for {used_minutes} minutes today, reaching the daily limit of {limit_minutes} minutes."
            ),
        ),
        (false, _) => (
            "Usage limit almost reached",
            format!(
                "{target} has been used for {used_minutes} minutes today, approaching the daily limit of {limit_minutes} minutes."
            ),
        ),
    }
}

fn format_tray_duration(duration_ms: i64) -> String {
    let total_minutes = duration_ms.max(0) / 60_000;
    let hours = total_minutes / 60;
    let minutes = total_minutes % 60;
    match (hours, minutes) {
        (0, minutes) => format!("{minutes}min"),
        (hours, 0) => format!("{hours}h"),
        (hours, minutes) => format!("{hours}h {minutes}min"),
    }
}

fn position_tray_panel(
    app: &tauri::AppHandle,
    panel: &WebviewWindow,
    click_position: PhysicalPosition<f64>,
    tray_rect: Rect,
) {
    let Ok(Some(monitor)) = app.monitor_from_point(click_position.x, click_position.y) else {
        return;
    };
    let scale_factor = monitor.scale_factor();
    let tray_position = tray_rect.position.to_physical::<f64>(scale_factor);
    let tray_size = tray_rect.size.to_physical::<f64>(scale_factor);
    let work_area = monitor.work_area();
    let work_left = work_area.position.x as f64;
    let work_top = work_area.position.y as f64;
    let work_right = work_left + work_area.size.width as f64;
    let work_bottom = work_top + work_area.size.height as f64;
    let panel_width = TRAY_PANEL_WIDTH * scale_factor;
    let panel_height = TRAY_PANEL_HEIGHT * scale_factor;
    let gap = 8.0 * scale_factor;
    let tray_center_x = tray_position.x + tray_size.width / 2.0;
    let tray_center_y = tray_position.y + tray_size.height / 2.0;
    let work_center_y = work_top + work_area.size.height as f64 / 2.0;

    let max_x = (work_right - panel_width).max(work_left);
    let max_y = (work_bottom - panel_height).max(work_top);
    let x = (tray_center_x - panel_width / 2.0).clamp(work_left, max_x);
    let preferred_y = if tray_center_y <= work_center_y {
        tray_position.y + tray_size.height + gap
    } else {
        tray_position.y - panel_height - gap
    };
    let y = preferred_y.clamp(work_top, max_y);
    let _ = panel.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

fn toggle_tray_panel(app: &tauri::AppHandle, position: PhysicalPosition<f64>, rect: Rect) {
    let Some(panel) = app.get_webview_window("tray-panel") else {
        return;
    };
    if panel.is_visible().unwrap_or(false) {
        let _ = panel.hide();
        return;
    }
    position_tray_panel(app, &panel, position, rect);
    let _ = app.emit_to("tray-panel", "tray-panel-shown", ());
    let _ = panel.show();
    let _ = panel.set_focus();
}

pub(crate) fn update_tray_overview(app: &tauri::AppHandle) {
    let chinese = app.state::<AppLocaleState>().is_chinese();
    let statistics = app.state::<statistics::StatisticsService>();

    let active_label = statistics
        .today_summary()
        .map(|summary| {
            let duration = format_tray_duration(summary.active_duration_ms);
            if chinese {
                format!("今日活跃：{duration}")
            } else {
                format!("Today active: {duration}")
            }
        })
        .unwrap_or_else(|_| {
            if chinese {
                "今日活跃：不可用".to_owned()
            } else {
                "Today active: unavailable".to_owned()
            }
        });
    let _ = app
        .state::<TodayActiveTrayMenuItem>()
        .0
        .set_text(active_label);

    let current = if app.state::<activity::MonitorHandle>().is_paused() {
        if chinese { "已暂停" } else { "Paused" }.to_owned()
    } else {
        match app.state::<activity::MonitorHandle>().current_status() {
            activity::MonitorStatus::Running(sample) => sample
                .foreground_application
                .map(|application| application.name)
                .unwrap_or_else(|| if chinese { "空闲" } else { "Idle" }.to_owned()),
            activity::MonitorStatus::Starting => {
                if chinese { "正在启动" } else { "Starting" }.to_owned()
            }
            activity::MonitorStatus::Paused => if chinese { "已暂停" } else { "Paused" }.to_owned(),
            activity::MonitorStatus::Degraded { .. } => if chinese {
                "暂不可用"
            } else {
                "Unavailable"
            }
            .to_owned(),
            activity::MonitorStatus::Stopped => {
                if chinese { "已停止" } else { "Stopped" }.to_owned()
            }
        }
    };
    let current_label = if chinese {
        format!("当前：{current}")
    } else {
        format!("Current: {current}")
    };
    let _ = app
        .state::<CurrentApplicationTrayMenuItem>()
        .0
        .set_text(current_label);

    let focus_label = statistics
        .today_focus_summary()
        .map(|summary| {
            let duration = format_tray_duration(summary.total_focus_duration_ms);
            if chinese {
                format!("今日专注：{duration}")
            } else {
                format!("Today focus: {duration}")
            }
        })
        .unwrap_or_else(|_| {
            if chinese {
                "今日专注：不可用".to_owned()
            } else {
                "Today focus: unavailable".to_owned()
            }
        });
    let _ = app
        .state::<TodayFocusTrayMenuItem>()
        .0
        .set_text(focus_label);

    let limit_label = statistics
        .today_usage_limit_progress()
        .ok()
        .and_then(|progress| {
            progress
                .into_iter()
                .filter(|item| item.enabled)
                .max_by(|left, right| left.percentage.total_cmp(&right.percentage))
        })
        .map(|item| {
            let target = item
                .application_name
                .as_deref()
                .or(item.category.as_deref())
                .unwrap_or("Watchhouse");
            let percentage = item.percentage.round().max(0.0) as i64;
            if chinese {
                format!("最接近限额：{target} {percentage}%")
            } else {
                format!("Closest limit: {target} {percentage}%")
            }
        })
        .unwrap_or_else(|| {
            if chinese {
                "最接近限额：未设置".to_owned()
            } else {
                "Closest limit: none".to_owned()
            }
        });
    let _ = app
        .state::<UsageLimitTrayMenuItem>()
        .0
        .set_text(limit_label);
}

#[cfg(not(debug_assertions))]
fn spawn_automatic_update_check(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(20)).await;
        let Ok(updater) = app.updater() else {
            return;
        };
        match updater.check().await {
            Ok(Some(update)) => {
                let version = update.version;
                let _ = app.emit("update-available", &version);
                let chinese = app.state::<AppLocaleState>().is_chinese();
                let _ = app
                    .notification()
                    .builder()
                    .title(if chinese {
                        "Watchhouse 有可用更新"
                    } else {
                        "Watchhouse update available"
                    })
                    .body(if chinese {
                        format!("版本 {version} 已可安装，请在设置中查看。")
                    } else {
                        format!("Version {version} is ready. Open Settings to install it.")
                    })
                    .show();
            }
            Ok(None) => {}
            Err(error) => log::warn!("automatic update check failed: {error}"),
        }
    });
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let app = tauri::Builder::default()
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(1_048_576)
                .build(),
        )
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if app.state::<DatabaseMaintenanceState>().is_active() {
                        return;
                    }
                    let configured = app.state::<ShortcutSettingsState>().snapshot();
                    let toggle = parse_shortcut(&configured.toggle_focus).ok().flatten();
                    let pause = parse_shortcut(&configured.pause_focus).ok().flatten();
                    let template = parse_shortcut(&configured.start_template).ok().flatten();
                    let current = app.state::<focus::FocusModeState>().snapshot();
                    let result = if toggle.as_ref() == Some(shortcut) {
                        commands::focus::set_focus_mode(
                            !current.active,
                            app.clone(),
                            app.state(),
                            app.state(),
                        )
                    } else if pause.as_ref() == Some(shortcut) && current.active {
                        commands::focus::set_focus_plan_paused(
                            !current.paused,
                            app.clone(),
                            app.state(),
                            app.state(),
                        )
                    } else if template.as_ref() == Some(shortcut) && !current.active {
                        app.state::<database::ActivityRepository>()
                            .focus_plan_templates()
                            .map_err(|error| error.to_string())
                            .and_then(|templates| {
                                templates
                                    .first()
                                    .ok_or_else(|| "no focus template is available".to_owned())
                                    .and_then(|template| {
                                        commands::focus::start_focus_template(
                                            template.id,
                                            app.clone(),
                                            app.state(),
                                            app.state(),
                                        )
                                    })
                            })
                    } else {
                        return;
                    };
                    if let Err(error) = result {
                        log::warn!("global focus shortcut failed: {error}");
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            log::info!(
                "YunQi-Watchhouse {} starting; local diagnostics enabled",
                app.package_info().version
            );
            let app_data = app.path().app_data_dir()?;
            let database_path = app_data.join("watchhouse.sqlite3");
            let database = database::Database::open(&database_path)?;
            let repository = database::ActivityRepository::new(database);
            repository.recover_open_session()?;
            let settings = repository.settings()?;
            let shortcut_settings = repository.shortcut_settings()?;
            let persisted_focus = repository.focus_mode_status()?;
            app.manage(statistics::StatisticsService::new(repository.clone()));
            app.manage(repository);
            app.manage(focus::FocusModeState::new(
                persisted_focus.active,
                persisted_focus.started_at_ms,
                persisted_focus.planned_end_at_ms,
                persisted_focus.paused,
                persisted_focus.paused_at_ms,
                persisted_focus.total_paused_ms,
                persisted_focus.template_id,
            ));
            app.manage(AppLocaleState::new());
            app.manage(ShortcutSettingsState::new(shortcut_settings.clone()));
            app.manage(DatabaseMaintenanceState::default());
            app.manage(commands::restore_preview::PreparedRestoreState::default());
            #[cfg(not(debug_assertions))]
            spawn_automatic_update_check(app.handle().clone());

            let reminder_repository = app.state::<database::ActivityRepository>().inner().clone();
            let reminder_state = app.state::<focus::FocusModeState>().inner().clone();
            let reminder_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                interval.tick().await;
                loop {
                    interval.tick().await;
                    if reminder_app.state::<DatabaseMaintenanceState>().is_active() {
                        continue;
                    }
                    let Ok(settings) = reminder_repository.settings() else {
                        continue;
                    };
                    let now_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_millis() as i64)
                        .unwrap_or_default();
                    send_usage_limit_notifications(&reminder_app, &reminder_repository, now_ms);
                    if reminder_state.is_due(now_ms) {
                        let current = reminder_state.snapshot();
                        if let Some(started_at_ms) = current.started_at_ms {
                            let _ = reminder_repository.record_focus_plan_outcome(
                                started_at_ms,
                                current.planned_end_at_ms,
                                now_ms,
                                current.total_paused_ms,
                                true,
                                current.template_id,
                            );
                        }
                        if reminder_repository
                            .update_focus_mode(
                                &database::PersistedFocusMode {
                                    active: false,
                                    started_at_ms: None,
                                    planned_end_at_ms: None,
                                    paused: false,
                                    paused_at_ms: None,
                                    total_paused_ms: 0,
                                    template_id: None,
                                },
                                now_ms,
                            )
                            .is_ok()
                        {
                            let next = reminder_state.end();
                            let _ = reminder_app.emit("focus-mode-changed", &next);
                            if let Some(item) = reminder_app.try_state::<FocusTrayMenuItem>() {
                                let chinese = reminder_app.state::<AppLocaleState>().is_chinese();
                                let _ = item.0.set_text(if chinese {
                                    "开始专注模式"
                                } else {
                                    "Start Focus Mode"
                                });
                            }
                            let chinese = reminder_app.state::<AppLocaleState>().is_chinese();
                            let _ = reminder_app
                                .notification()
                                .builder()
                                .title(if chinese {
                                    "专注计划已完成"
                                } else {
                                    "Focus plan complete"
                                })
                                .body(if chinese {
                                    "你的专注计划已经完成。"
                                } else {
                                    "Your planned focus session is complete."
                                })
                                .show();
                        }
                        continue;
                    }
                    if !settings.break_reminders_enabled {
                        continue;
                    }
                    let local_time = chrono::Local::now().format("%H:%M").to_string();
                    if focus::is_quiet_hours(
                        &local_time,
                        &settings.quiet_hours_start,
                        &settings.quiet_hours_end,
                    ) {
                        continue;
                    }
                    if reminder_state
                        .should_send_break_reminder(now_ms, settings.break_reminder_minutes)
                    {
                        let chinese = reminder_app.state::<AppLocaleState>().is_chinese();
                        if let Err(error) = reminder_app
                            .notification()
                            .builder()
                            .title(if chinese {
                                "该休息一下了"
                            } else {
                                "Time for a short break"
                            })
                            .body(if chinese {
                                "在下一个专注块开始前，暂时离开电脑休息一下。"
                            } else {
                                "Step away for a moment before your next focus block."
                            })
                            .show()
                        {
                            log::warn!("could not show break notification: {error}");
                        }
                    }
                }
            });

            let maintenance_status = maintenance::MaintenanceStatusState::default();
            app.manage(maintenance_status.clone());
            let maintenance_repository =
                app.state::<database::ActivityRepository>().inner().clone();
            let maintenance_statistics =
                app.state::<statistics::StatisticsService>().inner().clone();
            let maintenance_app_data = app_data.clone();
            let maintenance_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
                loop {
                    interval.tick().await;
                    let Ok(maintenance_guard) = maintenance_app
                        .state::<DatabaseMaintenanceState>()
                        .try_begin()
                    else {
                        continue;
                    };
                    let repository = maintenance_repository.clone();
                    let statistics = maintenance_statistics.clone();
                    let app_data = maintenance_app_data.clone();
                    maintenance_status.start();
                    let result = tauri::async_runtime::spawn_blocking(move || {
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_err(|error| error.to_string())?
                            .as_millis() as i64;
                        maintenance::run_due(&repository, &app_data, now_ms)?;
                        let notification = maintenance::archive_due_weekly_report(
                            &repository,
                            &statistics,
                            now_ms,
                        )?;
                        Ok::<_, String>(notification)
                    })
                    .await;
                    let completed_at_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_millis() as i64)
                        .unwrap_or_default();
                    let (status_result, notification) = match result {
                        Ok(Ok(notification)) => (Ok(()), notification),
                        Ok(Err(error)) => (Err(error), None),
                        Err(error) => (Err(error.to_string()), None),
                    };
                    maintenance_status.finish(completed_at_ms, &status_result);
                    drop(maintenance_guard);
                    if let Err(error) = status_result {
                        log::error!("automatic data maintenance failed: {error}");
                    }
                    if let Some(notification) = notification
                        && let Err(error) = commands::weekly_reports::notify_weekly_report_archive(
                            maintenance_app.clone(),
                            maintenance_repository.clone(),
                            notification.week_start_date,
                        )
                        .await
                    {
                        log::warn!("automatic weekly report notification failed: {error}");
                    }
                }
            });

            #[cfg(target_os = "macos")]
            {
                let (session_manager, sample_sender) = activity::SessionManager::new(
                    app.state::<database::ActivityRepository>().inner().clone(),
                    activity::SessionManagerConfig::default(),
                )?
                .spawn_with_notifier({
                    let app = app.handle().clone();
                    move || {
                        let _ = app.emit("activity-data-changed", ());
                    }
                });
                let monitor = activity::ActivityMonitor::new(
                    platform::MacOsActivityProvider,
                    activity::MonitorConfig {
                        idle_threshold: std::time::Duration::from_secs(
                            settings.idle_threshold_seconds as u64,
                        ),
                        ..activity::MonitorConfig::default()
                    },
                )?
                .with_window_title_policy({
                    let repository = app.state::<database::ActivityRepository>().inner().clone();
                    move |application| {
                        let identity =
                            if let Some(bundle) = application.bundle_identifier.as_deref() {
                                format!("bundle:{bundle}")
                            } else if let Some(path) = application.executable_path.as_deref() {
                                format!("path:{path}")
                            } else {
                                format!("name:{}", application.name)
                            };
                        repository
                            .should_record_window_title(&identity)
                            .unwrap_or(false)
                    }
                })
                .spawn_with_sample_sink(sample_sender);
                if !settings.start_tracking_automatically || !settings.onboarding_completed {
                    monitor.set_paused(true);
                    if let Some(receiver) = session_manager.request_pause() {
                        tauri::async_runtime::spawn(async move {
                            let _ = receiver.await;
                        });
                    }
                }
                app.manage(session_manager);
                app.manage(monitor);
            }

            WebviewWindowBuilder::new(app, "tray-panel", WebviewUrl::App("index.html".into()))
                .title("Watchhouse")
                .inner_size(TRAY_PANEL_WIDTH, TRAY_PANEL_HEIGHT)
                .min_inner_size(TRAY_PANEL_WIDTH, TRAY_PANEL_HEIGHT)
                .max_inner_size(TRAY_PANEL_WIDTH, TRAY_PANEL_HEIGHT)
                .resizable(false)
                .decorations(false)
                .always_on_top(true)
                .visible_on_all_workspaces(true)
                .skip_taskbar(true)
                .shadow(true)
                .visible(false)
                .focused(false)
                .build()?;

            let show = MenuItem::with_id(app, "show", "Show Watchhouse", true, None::<&str>)?;
            app.manage(ShowTrayMenuItem(show.clone()));
            let pause = MenuItem::with_id(
                app,
                "pause",
                if settings.start_tracking_automatically && settings.onboarding_completed {
                    "Pause Tracking"
                } else {
                    "Resume Tracking"
                },
                true,
                None::<&str>,
            )?;
            app.manage(TrackingTrayMenuItem(pause.clone()));
            let focus = MenuItem::with_id(
                app,
                "focus",
                if persisted_focus.active {
                    "End Focus Mode"
                } else {
                    "Start Focus Mode"
                },
                true,
                None::<&str>,
            )?;
            app.manage(FocusTrayMenuItem(focus.clone()));
            let countdown = MenuItem::with_id(
                app,
                "focus-countdown",
                "No active focus plan",
                false,
                None::<&str>,
            )?;
            app.manage(FocusCountdownTrayMenuItem(countdown.clone()));
            let today_active = MenuItem::with_id(
                app,
                "today-active-summary",
                "Today active: 0min",
                false,
                None::<&str>,
            )?;
            app.manage(TodayActiveTrayMenuItem(today_active.clone()));
            let current_application = MenuItem::with_id(
                app,
                "current-application-summary",
                "Current: Starting",
                false,
                None::<&str>,
            )?;
            app.manage(CurrentApplicationTrayMenuItem(current_application.clone()));
            let today_focus = MenuItem::with_id(
                app,
                "today-focus-summary",
                "Today focus: 0min",
                false,
                None::<&str>,
            )?;
            app.manage(TodayFocusTrayMenuItem(today_focus.clone()));
            let usage_limit = MenuItem::with_id(
                app,
                "usage-limit-summary",
                "Closest limit: none",
                false,
                None::<&str>,
            )?;
            app.manage(UsageLimitTrayMenuItem(usage_limit.clone()));
            let templates = app
                .state::<database::ActivityRepository>()
                .focus_plan_templates()
                .unwrap_or_default();
            let template_items = templates
                .iter()
                .take(5)
                .map(|template| {
                    MenuItem::with_id(
                        app,
                        format!("focus-template-{}", template.id),
                        format!(
                            "Start: {} ({} min)",
                            template.name, template.duration_minutes
                        ),
                        true,
                        None::<&str>,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            app.manage(FocusTemplateTrayMenuItems(template_items.clone()));
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            app.manage(QuitTrayMenuItem(quit.clone()));
            let overview_separator = PredefinedMenuItem::separator(app)?;
            let focus_separator = PredefinedMenuItem::separator(app)?;
            let quit_separator = PredefinedMenuItem::separator(app)?;
            let mut menu_items: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = vec![
                &today_active,
                &current_application,
                &today_focus,
                &usage_limit,
                &overview_separator,
                &show,
                &pause,
                &focus_separator,
                &focus,
                &countdown,
            ];
            menu_items.extend(
                template_items
                    .iter()
                    .map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>),
            );
            menu_items.push(&quit_separator);
            menu_items.push(&quit);
            let menu = Menu::with_items(app, &menu_items)?;
            TrayIconBuilder::with_id("watchhouse-tray")
                .icon(app.default_window_icon().expect("application icon").clone())
                .tooltip("YunQi-Watchhouse")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        position,
                        rect,
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        toggle_tray_panel(tray.app_handle(), position, rect);
                    }
                })
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.unminimize();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    "pause" => {
                        let monitor = app.state::<activity::MonitorHandle>();
                        let paused = !monitor.is_paused();
                        if !paused && app.state::<DatabaseMaintenanceState>().is_active() {
                            return;
                        }
                        if paused {
                            monitor.set_paused(true);
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                app.state::<activity::SessionManagerHandle>().pause().await;
                            });
                        } else {
                            monitor.set_paused(false);
                        }
                        let chinese = app.state::<AppLocaleState>().is_chinese();
                        let label = if paused && chinese {
                            "继续追踪"
                        } else if paused {
                            "Resume Tracking"
                        } else if chinese {
                            "暂停追踪"
                        } else {
                            "Pause Tracking"
                        };
                        let _ = app.state::<TrackingTrayMenuItem>().0.set_text(label);
                    }
                    "focus" => {
                        if app.state::<DatabaseMaintenanceState>().is_active() {
                            return;
                        }
                        let state = app.state::<focus::FocusModeState>();
                        let current = state.snapshot();
                        let active = !current.active;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|duration| duration.as_millis() as i64)
                            .unwrap_or_default();
                        if let Err(error) = app
                            .state::<database::ActivityRepository>()
                            .update_focus_mode(
                                &database::PersistedFocusMode {
                                    active,
                                    started_at_ms: active.then_some(now_ms),
                                    planned_end_at_ms: None,
                                    paused: false,
                                    paused_at_ms: None,
                                    total_paused_ms: 0,
                                    template_id: None,
                                },
                                now_ms,
                            )
                        {
                            log::error!("could not persist focus mode state: {error}");
                            return;
                        }
                        if !active && let Some(started_at_ms) = current.started_at_ms {
                            let _ = app
                                .state::<database::ActivityRepository>()
                                .record_focus_plan_outcome(
                                    started_at_ms,
                                    current.planned_end_at_ms,
                                    now_ms,
                                    current.total_paused_ms,
                                    false,
                                    current.template_id,
                                );
                        }
                        let status = if active {
                            state.start(now_ms, None, None)
                        } else {
                            state.end()
                        };
                        let chinese = app.state::<AppLocaleState>().is_chinese();
                        let _ = app
                            .state::<FocusTrayMenuItem>()
                            .0
                            .set_text(if active && chinese {
                                "结束专注模式"
                            } else if active {
                                "End Focus Mode"
                            } else if chinese {
                                "开始专注模式"
                            } else {
                                "Start Focus Mode"
                            });
                        let _ = app.emit("focus-mode-changed", status);
                    }
                    id if id.starts_with("focus-template-") => {
                        let Ok(template_id) = id["focus-template-".len()..].parse::<i64>() else {
                            return;
                        };
                        if let Err(error) = commands::focus::start_focus_template(
                            template_id,
                            app.clone(),
                            app.state(),
                            app.state(),
                        ) {
                            log::warn!("could not start focus template from tray: {error}");
                        }
                    }
                    _ => {}
                })
                .build(app)?;
            update_tray_overview(app.handle());
            let overview_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                interval.tick().await;
                loop {
                    interval.tick().await;
                    update_tray_overview(&overview_app);
                }
            });
            for shortcut in [
                parse_shortcut(&shortcut_settings.toggle_focus)?,
                parse_shortcut(&shortcut_settings.pause_focus)?,
                parse_shortcut(&shortcut_settings.start_template)?,
            ]
            .into_iter()
            .flatten()
            {
                if let Err(error) = app.global_shortcut().register(shortcut) {
                    log::warn!("could not register global shortcut: {error}");
                }
            }
            let countdown_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    let status = countdown_app.state::<focus::FocusModeState>().snapshot();
                    let chinese = countdown_app.state::<AppLocaleState>().is_chinese();
                    let label = if !status.active && chinese {
                        "没有进行中的专注计划".to_owned()
                    } else if !status.active {
                        "No active focus plan".to_owned()
                    } else if status.paused && chinese {
                        "专注已暂停".to_owned()
                    } else if status.paused {
                        "Focus paused".to_owned()
                    } else if let Some(end_at) = status.planned_end_at_ms {
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|duration| duration.as_millis() as i64)
                            .unwrap_or_default();
                        let remaining = end_at.saturating_sub(now_ms);
                        if chinese {
                            format!(
                                "专注剩余：{:02}:{:02}",
                                remaining / 60_000,
                                (remaining / 1_000) % 60
                            )
                        } else {
                            format!(
                                "Focus remaining: {:02}:{:02}",
                                remaining / 60_000,
                                (remaining / 1_000) % 60
                            )
                        }
                    } else if chinese {
                        "专注进行中".to_owned()
                    } else {
                        "Focus active".to_owned()
                    };
                    let _ = countdown_app
                        .state::<FocusCountdownTrayMenuItem>()
                        .0
                        .set_text(label);
                }
            });
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "tray-panel" {
                match event {
                    tauri::WindowEvent::CloseRequested { api, .. } => {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                    tauri::WindowEvent::Focused(false) => {
                        let _ = window.hide();
                    }
                    _ => {}
                }
                return;
            }
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let hide = window
                    .state::<database::ActivityRepository>()
                    .settings()
                    .map(|settings| settings.hide_to_tray_on_close)
                    .unwrap_or(false);
                if hide {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::focus::get_focus_mode,
            commands::focus::get_focus_plan_history,
            commands::focus::get_focus_plan_templates,
            commands::focus::create_focus_plan_template,
            commands::focus::update_focus_plan_template,
            commands::focus::delete_focus_plan_template,
            commands::focus::start_focus_template,
            commands::focus::set_focus_mode,
            commands::focus::start_focus_plan,
            commands::focus::set_focus_plan_paused,
            commands::focus::end_focus_plan,
            commands::activity::get_current_activity,
            commands::activity::set_tracking_paused,
            commands::applications::get_application_icon,
            commands::applications::clear_application_icon_cache,
            commands::applications::update_application_preferences,
            commands::category_rules::get_category_rules,
            commands::category_rules::preview_category_rule,
            commands::category_rules::create_category_rule,
            commands::category_rules::update_category_rule,
            commands::category_rules::delete_category_rule,
            commands::category_rules::reorder_category_rules,
            commands::category_rules::preview_category_rules_reapply,
            commands::category_rules::reapply_category_rules,
            commands::category_rules::get_category_rules_reapply_undo_status,
            commands::category_rules::undo_category_rules_reapply,
            commands::projects::list_projects,
            commands::projects::create_project,
            commands::projects::update_project,
            commands::projects::set_project_archived,
            commands::projects::list_activity_tags,
            commands::projects::create_activity_tag,
            commands::projects::update_activity_tag,
            commands::projects::set_activity_tag_archived,
            commands::projects::get_session_organization,
            commands::projects::set_session_organization,
            commands::projects::set_sessions_organization,
            commands::statistics::get_today_summary,
            commands::statistics::get_today_focus_summary,
            commands::statistics::get_timeline,
            commands::statistics::get_timeline_page,
            commands::statistics::search_timeline_range,
            commands::statistics::get_app_usage,
            commands::statistics::get_category_usage,
            commands::statistics::get_daily_usage,
            commands::statistics::get_productivity_report,
            commands::statistics::export_productivity_report_csv,
            commands::statistics::get_application_daily_usage,
            commands::weekly_reports::archive_weekly_report,
            commands::weekly_reports::get_weekly_report_archives,
            commands::weekly_reports::delete_weekly_report_archive,
            commands::weekly_reports::send_weekly_report_notification,
            commands::usage_limits::get_usage_limits,
            commands::usage_limits::get_usage_limit_targets,
            commands::usage_limits::create_usage_limit,
            commands::usage_limits::update_usage_limit,
            commands::usage_limits::delete_usage_limit,
            commands::usage_limits::get_today_usage_limit_progress,
            commands::usage_limits::get_usage_limit_reminder_history,
            commands::usage_limits::snooze_usage_limit_notifications,
            commands::usage_limits::silence_usage_limit_notifications_for_today,
            commands::usage_limits::add_temporary_usage_limit_minutes,
            commands::usage_limits::clear_temporary_usage_limit_minutes,
            commands::timeline::delete_timeline_session,
            commands::timeline::delete_timeline_sessions,
            commands::timeline::merge_timeline_sessions,
            commands::timeline::split_timeline_session,
            commands::timeline::update_timeline_session_notes,
            commands::timeline::update_timeline_session_categories,
            commands::timeline::undo_timeline_edit,
            commands::timeline::get_timeline_undo_tokens,
            commands::timeline::get_timeline_undo_history,
            commands::timeline::update_timeline_session,
            commands::timeline::preview_activity_import,
            commands::timeline::import_activity,
            commands::settings::get_settings,
            commands::settings::set_app_locale,
            commands::settings::get_shortcut_settings,
            commands::settings::update_shortcut_settings,
            commands::settings::get_data_health_summary,
            commands::settings::repair_data_health,
            commands::settings::get_data_health_undo_status,
            commands::settings::undo_data_health_repair,
            commands::settings::get_accessibility_permission,
            commands::settings::get_notification_permission,
            commands::settings::request_notification_permission,
            commands::settings::send_test_notification,
            commands::settings::complete_onboarding,
            commands::settings::update_settings,
            commands::settings::delete_all_activity,
            commands::settings::export_activity,
            commands::settings::open_data_directory,
            commands::settings::open_log_directory,
            commands::settings::get_diagnostics_summary,
            commands::settings::run_diagnostics_repair,
            commands::settings::backup_database,
            commands::encrypted_backup::create_encrypted_database_backup,
            commands::encrypted_backup::set_automatic_encrypted_backup_password,
            commands::encrypted_backup::clear_automatic_encrypted_backup_password,
            commands::encrypted_backup::has_automatic_encrypted_backup_password,
            commands::restore_preview::preview_database_restore,
            commands::restore_preview::preview_encrypted_database_restore,
            commands::restore_preview::cancel_prepared_database_restore,
            commands::restore_preview::restore_prepared_database,
            commands::settings::choose_backup_directory,
            commands::settings::open_backup_directory,
            commands::settings::get_maintenance_preview,
            commands::settings::get_maintenance_status,
            commands::settings::run_data_maintenance,
            commands::settings::create_automatic_backup_now,
            commands::settings::optimize_database,
            commands::updater::check_for_updates,
            commands::updater::install_update,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|app_handle, event| {
        if matches!(event, tauri::RunEvent::ExitRequested { .. }) {
            tauri::async_runtime::block_on(async {
                if let Some(monitor) = app_handle.try_state::<activity::MonitorHandle>() {
                    monitor.shutdown().await;
                }
                if let Some(manager) = app_handle.try_state::<activity::SessionManagerHandle>() {
                    manager.shutdown().await;
                }
            });
        }
    });
}

#[cfg(test)]
mod shortcut_tests {
    use super::{
        DatabaseMaintenanceState, format_tray_duration, parse_shortcut,
        usage_limit_notification_copy,
    };

    #[test]
    fn database_maintenance_lease_is_exclusive_and_released_on_drop() {
        let state = DatabaseMaintenanceState::default();
        assert!(!state.is_active());
        let guard = state.try_begin().expect("first maintenance lease");
        assert!(state.is_active());
        assert!(state.try_begin().is_err());
        drop(guard);
        assert!(!state.is_active());
        assert!(state.try_begin().is_ok());
    }

    #[test]
    fn database_maintenance_lease_is_shared_across_threads() {
        let state = DatabaseMaintenanceState::default();
        let competing_state = state.clone();
        let guard = state.try_begin().expect("automatic maintenance lease");

        let was_rejected = std::thread::spawn(move || competing_state.try_begin().is_err())
            .join()
            .expect("competing maintenance thread should finish");

        assert!(was_rejected);
        drop(guard);
        assert!(state.try_begin().is_ok());
    }

    #[test]
    fn shortcut_parser_accepts_presets_and_disabled_actions() {
        assert!(
            parse_shortcut(&Some("CommandOrControl+Shift+F".to_owned()))
                .unwrap()
                .is_some()
        );
        assert!(parse_shortcut(&None).unwrap().is_none());
        assert!(parse_shortcut(&Some("not a shortcut".to_owned())).is_err());
    }

    #[test]
    fn usage_limit_notification_copy_tracks_locale_and_threshold() {
        let (title, body) = usage_limit_notification_copy("IDEA", 48 * 60_000, 60, 80, true);
        assert_eq!(title, "接近使用限额");
        assert!(body.contains("48"));
        let (title, body) = usage_limit_notification_copy("IDEA", 61 * 60_000, 60, 100, false);
        assert_eq!(title, "Usage limit reached");
        assert!(body.contains("61"));
    }

    #[test]
    fn tray_duration_stays_compact_in_both_locales() {
        assert_eq!(format_tray_duration(45 * 60_000), "45min");
        assert_eq!(format_tray_duration(2 * 60 * 60_000), "2h");
        assert_eq!(format_tray_duration(135 * 60_000), "2h 15min");
    }
}
