use serde::Serialize;
use tauri::State;

use crate::{
    TrackingTrayMenuItem,
    activity::{MonitorHandle, MonitorStatus, SessionManagerHandle, SessionManagerStatus},
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentActivityResponse {
    pub monitor: MonitorStatus,
    pub persistence: SessionManagerStatus,
    pub paused: bool,
}

#[tauri::command]
pub fn get_current_activity(
    monitor: State<'_, MonitorHandle>,
    session_manager: State<'_, SessionManagerHandle>,
) -> CurrentActivityResponse {
    CurrentActivityResponse {
        monitor: monitor.current_status(),
        persistence: session_manager.current_status(),
        paused: monitor.is_paused(),
    }
}

#[tauri::command]
pub async fn set_tracking_paused(
    paused: bool,
    monitor: State<'_, MonitorHandle>,
    session_manager: State<'_, SessionManagerHandle>,
    tray_item: State<'_, TrackingTrayMenuItem>,
) -> Result<bool, String> {
    let acknowledgement = if paused {
        monitor.set_paused(true);
        session_manager.request_pause()
    } else {
        monitor.set_paused(false);
        None
    };
    let is_paused = monitor.is_paused();
    let _ = tray_item.0.set_text(if is_paused {
        "Resume Tracking"
    } else {
        "Pause Tracking"
    });
    if paused && let Some(receiver) = acknowledgement {
        let _ = receiver.await;
    }
    Ok(is_paused)
}
