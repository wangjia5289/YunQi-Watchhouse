use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Local, TimeZone};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::{
    FocusTrayMenuItem,
    database::{ActivityRepository, FocusPlanTemplate, PersistedFocusMode},
    focus::{FocusModeState, FocusModeStatus},
};

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusPlanHistorySummary {
    completed_count: usize,
    cancelled_count: usize,
    total_planned_duration_ms: i64,
    total_actual_duration_ms: i64,
    total_paused_duration_ms: i64,
    longest_completed_streak_days: usize,
    recent_plans: Vec<crate::database::FocusPlanHistoryEntry>,
}

#[tauri::command]
pub fn get_focus_plan_templates(
    repository: State<'_, ActivityRepository>,
) -> Result<Vec<FocusPlanTemplate>, String> {
    repository
        .focus_plan_templates()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn create_focus_plan_template(
    name: String,
    duration_minutes: i64,
    repository: State<'_, ActivityRepository>,
) -> Result<FocusPlanTemplate, String> {
    repository
        .create_focus_plan_template(&name, duration_minutes)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn update_focus_plan_template(
    template_id: i64,
    name: String,
    duration_minutes: i64,
    sort_order: i64,
    repository: State<'_, ActivityRepository>,
) -> Result<FocusPlanTemplate, String> {
    repository
        .update_focus_plan_template(template_id, &name, duration_minutes, sort_order)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn delete_focus_plan_template(
    template_id: i64,
    repository: State<'_, ActivityRepository>,
) -> Result<(), String> {
    repository
        .delete_focus_plan_template(template_id)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_focus_mode(status: State<'_, FocusModeState>) -> FocusModeStatus {
    status.snapshot()
}

#[tauri::command]
pub fn get_focus_plan_history(
    range_start_ms: i64,
    range_end_ms: i64,
    repository: State<'_, ActivityRepository>,
) -> Result<FocusPlanHistorySummary, String> {
    if range_end_ms <= range_start_ms {
        return Err("focus history range end must be after its start".to_owned());
    }
    let mut recent_plans = repository
        .focus_plan_history(range_start_ms, range_end_ms)
        .map_err(|error| error.to_string())?;
    let completed_count = recent_plans
        .iter()
        .filter(|entry| entry.outcome == "COMPLETED")
        .count();
    let cancelled_count = recent_plans.len().saturating_sub(completed_count);
    let total_planned_duration_ms = recent_plans
        .iter()
        .filter_map(|entry| {
            entry
                .planned_end_at_ms
                .map(|end| end.saturating_sub(entry.started_at_ms).max(0))
        })
        .sum();
    let total_actual_duration_ms = recent_plans
        .iter()
        .map(|entry| {
            entry
                .ended_at_ms
                .saturating_sub(entry.started_at_ms)
                .saturating_sub(entry.paused_duration_ms)
                .max(0)
        })
        .sum();
    let total_paused_duration_ms = recent_plans
        .iter()
        .map(|entry| entry.paused_duration_ms.max(0))
        .sum();
    let completed_dates = recent_plans
        .iter()
        .filter(|entry| entry.outcome == "COMPLETED")
        .filter_map(|entry| {
            Local
                .timestamp_millis_opt(entry.ended_at_ms)
                .single()
                .map(|ended_at| ended_at.date_naive())
        })
        .collect::<Vec<_>>();
    let longest_completed_streak_days = longest_completed_streak(&completed_dates);
    recent_plans.truncate(30);
    Ok(FocusPlanHistorySummary {
        completed_count,
        cancelled_count,
        total_planned_duration_ms,
        total_actual_duration_ms,
        total_paused_duration_ms,
        longest_completed_streak_days,
        recent_plans,
    })
}

fn longest_completed_streak(dates: &[chrono::NaiveDate]) -> usize {
    let mut dates = dates.to_vec();
    dates.sort_unstable();
    dates.dedup();
    let mut longest = 0;
    let mut current = 0;
    let mut previous = None;
    for date in dates {
        current = match previous {
            Some(previous) if date.signed_duration_since(previous).num_days() == 1 => current + 1,
            _ => 1,
        };
        longest = longest.max(current);
        previous = Some(date);
    }
    longest
}

#[tauri::command]
pub fn set_focus_mode(
    active: bool,
    app: AppHandle,
    status: State<'_, FocusModeState>,
    repository: State<'_, ActivityRepository>,
) -> Result<FocusModeStatus, String> {
    if active {
        start_focus_plan(None, app, status, repository)
    } else {
        end_focus_plan(false, app, status, repository)
    }
}

#[tauri::command]
pub fn start_focus_plan(
    duration_minutes: Option<i64>,
    app: AppHandle,
    status: State<'_, FocusModeState>,
    repository: State<'_, ActivityRepository>,
) -> Result<FocusModeStatus, String> {
    start_focus_plan_with_template(duration_minutes, None, app, status, repository)
}

#[tauri::command]
pub fn start_focus_template(
    template_id: i64,
    app: AppHandle,
    status: State<'_, FocusModeState>,
    repository: State<'_, ActivityRepository>,
) -> Result<FocusModeStatus, String> {
    let template = repository
        .focus_plan_templates()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|template| template.id == template_id)
        .ok_or_else(|| "focus template was not found".to_owned())?;
    start_focus_plan_with_template(
        Some(template.duration_minutes),
        Some(template.id),
        app,
        status,
        repository,
    )
}

fn start_focus_plan_with_template(
    duration_minutes: Option<i64>,
    template_id: Option<i64>,
    app: AppHandle,
    status: State<'_, FocusModeState>,
    repository: State<'_, ActivityRepository>,
) -> Result<FocusModeStatus, String> {
    if duration_minutes.is_some_and(|minutes| !(5..=240).contains(&minutes)) {
        return Err("focus plan duration must be between 5 and 240 minutes".to_owned());
    }
    let now_ms = unix_timestamp_ms()?;
    let planned_end_at_ms =
        duration_minutes.map(|minutes| now_ms.saturating_add(minutes.saturating_mul(60_000)));
    repository
        .update_focus_mode(
            &PersistedFocusMode {
                active: true,
                started_at_ms: Some(now_ms),
                planned_end_at_ms,
                paused: false,
                paused_at_ms: None,
                total_paused_ms: 0,
                template_id,
            },
            now_ms,
        )
        .map_err(|error| error.to_string())?;
    if let Some(template_id) = template_id {
        repository
            .mark_focus_template_started(template_id, now_ms)
            .map_err(|error| error.to_string())?;
    }
    let next = status.start(now_ms, planned_end_at_ms, template_id);
    publish_status(&app, &next);
    Ok(next)
}

#[tauri::command]
pub fn set_focus_plan_paused(
    paused: bool,
    app: AppHandle,
    status: State<'_, FocusModeState>,
    repository: State<'_, ActivityRepository>,
) -> Result<FocusModeStatus, String> {
    let now_ms = unix_timestamp_ms()?;
    let next = status.set_paused(paused, now_ms);
    repository
        .update_focus_mode(&persisted(&next), now_ms)
        .map_err(|error| error.to_string())?;
    publish_status(&app, &next);
    Ok(next)
}

#[tauri::command]
pub fn end_focus_plan(
    completed: bool,
    app: AppHandle,
    status: State<'_, FocusModeState>,
    repository: State<'_, ActivityRepository>,
) -> Result<FocusModeStatus, String> {
    let now_ms = unix_timestamp_ms()?;
    let current = status.snapshot();
    if let Some(started_at_ms) = current.started_at_ms.filter(|_| current.active) {
        let current_pause = current
            .paused_at_ms
            .map(|paused_at| now_ms.saturating_sub(paused_at))
            .unwrap_or_default();
        repository
            .record_focus_plan_outcome(
                started_at_ms,
                current.planned_end_at_ms,
                now_ms,
                current.total_paused_ms.saturating_add(current_pause),
                completed,
                current.template_id,
            )
            .map_err(|error| error.to_string())?;
    }
    repository
        .update_focus_mode(
            &PersistedFocusMode {
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
        .map_err(|error| error.to_string())?;
    let next = status.end();
    publish_status(&app, &next);
    Ok(next)
}

fn unix_timestamp_ms() -> Result<i64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())
        .map(|duration| duration.as_millis() as i64)
}

fn publish_status(app: &AppHandle, status: &FocusModeStatus) {
    let _ = app
        .state::<FocusTrayMenuItem>()
        .0
        .set_text(if status.active {
            "End Focus Mode"
        } else {
            "Start Focus Mode"
        });
    let _ = app.emit("focus-mode-changed", status);
}

fn persisted(status: &FocusModeStatus) -> PersistedFocusMode {
    PersistedFocusMode {
        active: status.active,
        started_at_ms: status.started_at_ms,
        planned_end_at_ms: status.planned_end_at_ms,
        paused: status.paused,
        paused_at_ms: status.paused_at_ms,
        total_paused_ms: status.total_paused_ms,
        template_id: status.template_id,
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::longest_completed_streak;

    #[test]
    fn completed_streak_deduplicates_days_and_keeps_the_longest_run() {
        let date = |value| NaiveDate::parse_from_str(value, "%Y-%m-%d").unwrap();
        assert_eq!(
            longest_completed_streak(&[
                date("2026-07-03"),
                date("2026-07-01"),
                date("2026-07-02"),
                date("2026-07-02"),
                date("2026-07-06"),
                date("2026-07-07"),
            ]),
            3
        );
        assert_eq!(longest_completed_streak(&[]), 0);
    }
}
