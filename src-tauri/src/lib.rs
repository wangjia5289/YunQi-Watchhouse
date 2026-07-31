pub mod activity;
pub mod commands;
pub mod database;
pub mod error;
pub mod focus;
pub mod maintenance;
pub mod platform;
pub mod statistics;

use tauri::{Emitter, Manager};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
};

pub struct TrackingTrayMenuItem(pub MenuItem<tauri::Wry>);
pub struct FocusTrayMenuItem(pub MenuItem<tauri::Wry>);

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
            app.manage(statistics::StatisticsService::new(repository.clone()));
            app.manage(repository);
            app.manage(focus::FocusModeState::default());

            let maintenance_status = maintenance::MaintenanceStatusState::default();
            app.manage(maintenance_status.clone());
            let maintenance_repository =
                app.state::<database::ActivityRepository>().inner().clone();
            let maintenance_app_data = app_data.clone();
            tauri::async_runtime::spawn(async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60 * 60));
                loop {
                    interval.tick().await;
                    let repository = maintenance_repository.clone();
                    let app_data = maintenance_app_data.clone();
                    maintenance_status.start();
                    let result = tauri::async_runtime::spawn_blocking(move || {
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map_err(|error| error.to_string())?
                            .as_millis() as i64;
                        maintenance::run_due(&repository, &app_data, now_ms)
                    })
                    .await;
                    let completed_at_ms = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|duration| duration.as_millis() as i64)
                        .unwrap_or_default();
                    let status_result = match result {
                        Ok(result) => result,
                        Err(error) => Err(error.to_string()),
                    };
                    maintenance_status.finish(completed_at_ms, &status_result);
                    if let Err(error) = status_result {
                        log::error!("automatic data maintenance failed: {error}");
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

            let show = MenuItem::with_id(app, "show", "Show Watchhouse", true, None::<&str>)?;
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
            let focus = MenuItem::with_id(app, "focus", "Start Focus Mode", true, None::<&str>)?;
            app.manage(FocusTrayMenuItem(focus.clone()));
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &pause, &focus, &quit])?;
            TrayIconBuilder::with_id("watchhouse-tray")
                .icon(app.default_window_icon().expect("application icon").clone())
                .tooltip("YunQi-Watchhouse")
                .menu(&menu)
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
                        if paused {
                            monitor.set_paused(true);
                            let app = app.clone();
                            tauri::async_runtime::spawn(async move {
                                app.state::<activity::SessionManagerHandle>().pause().await;
                            });
                        } else {
                            monitor.set_paused(false);
                        }
                        let label = if paused {
                            "Resume Tracking"
                        } else {
                            "Pause Tracking"
                        };
                        let _ = app.state::<TrackingTrayMenuItem>().0.set_text(label);
                    }
                    "focus" => {
                        let state = app.state::<focus::FocusModeState>();
                        let active = !state.snapshot().active;
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|duration| duration.as_millis() as i64)
                            .unwrap_or_default();
                        let status = state.set_active(active, now_ms);
                        let _ = app.state::<FocusTrayMenuItem>().0.set_text(if active {
                            "End Focus Mode"
                        } else {
                            "Start Focus Mode"
                        });
                        let _ = app.emit("focus-mode-changed", status);
                    }
                    _ => {}
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
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
            commands::focus::set_focus_mode,
            commands::activity::get_current_activity,
            commands::activity::set_tracking_paused,
            commands::applications::get_application_icon,
            commands::applications::clear_application_icon_cache,
            commands::applications::update_application_preferences,
            commands::statistics::get_today_summary,
            commands::statistics::get_today_focus_summary,
            commands::statistics::get_timeline,
            commands::statistics::get_app_usage,
            commands::statistics::get_category_usage,
            commands::statistics::get_daily_usage,
            commands::statistics::get_application_daily_usage,
            commands::timeline::delete_timeline_session,
            commands::timeline::delete_timeline_sessions,
            commands::timeline::merge_timeline_sessions,
            commands::timeline::update_timeline_session_notes,
            commands::timeline::update_timeline_session_categories,
            commands::timeline::undo_timeline_edit,
            commands::timeline::update_timeline_session,
            commands::timeline::preview_activity_import,
            commands::timeline::import_activity,
            commands::settings::get_settings,
            commands::settings::get_accessibility_permission,
            commands::settings::complete_onboarding,
            commands::settings::update_settings,
            commands::settings::delete_all_activity,
            commands::settings::export_activity,
            commands::settings::open_data_directory,
            commands::settings::open_log_directory,
            commands::settings::get_diagnostics_summary,
            commands::settings::backup_database,
            commands::settings::choose_backup_directory,
            commands::settings::open_backup_directory,
            commands::settings::get_maintenance_preview,
            commands::settings::get_maintenance_status,
            commands::settings::run_data_maintenance,
            commands::settings::create_automatic_backup_now,
            commands::settings::restore_database,
            commands::settings::optimize_database,
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
