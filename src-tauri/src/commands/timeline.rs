use tauri::State;

use crate::database::ActivityRepository;

#[tauri::command]
pub fn delete_timeline_session(
    session_id: i64,
    repository: State<'_, ActivityRepository>,
) -> Result<(), String> {
    repository
        .delete_closed_session(session_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_timeline_session(
    session_id: i64,
    started_at_ms: i64,
    ended_at_ms: i64,
    repository: State<'_, ActivityRepository>,
) -> Result<(), String> {
    repository
        .update_closed_session_bounds(session_id, started_at_ms, ended_at_ms)
        .map(|_| ())
        .map_err(|error| error.to_string())
}
