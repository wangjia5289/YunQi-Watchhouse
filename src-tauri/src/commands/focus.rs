use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    FocusTrayMenuItem,
    database::ActivityRepository,
    focus::{FocusModeState, FocusModeStatus},
};

#[tauri::command]
pub fn get_focus_mode(status: State<'_, FocusModeState>) -> FocusModeStatus {
    status.snapshot()
}

#[tauri::command]
pub fn set_focus_mode(
    active: bool,
    app: AppHandle,
    status: State<'_, FocusModeState>,
    repository: State<'_, ActivityRepository>,
) -> Result<FocusModeStatus, String> {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis() as i64;
    repository
        .update_focus_mode(active, active.then_some(now_ms), now_ms)
        .map_err(|error| error.to_string())?;
    let status = status.set_active(active, now_ms);
    let _ = app.state::<FocusTrayMenuItem>().0.set_text(if active {
        "End Focus Mode"
    } else {
        "Start Focus Mode"
    });
    let _ = app.emit("focus-mode-changed", &status);
    Ok(status)
}
