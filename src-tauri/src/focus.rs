use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusModeStatus {
    pub active: bool,
    pub started_at_ms: Option<i64>,
    pub planned_end_at_ms: Option<i64>,
    pub paused: bool,
    pub paused_at_ms: Option<i64>,
    pub total_paused_ms: i64,
    pub template_id: Option<i64>,
}

#[derive(Default)]
struct FocusModeRuntime {
    status: FocusModeStatus,
    last_reminded_interval: i64,
}

#[derive(Clone, Default)]
pub struct FocusModeState(Arc<Mutex<FocusModeRuntime>>);

impl FocusModeState {
    pub fn new(
        active: bool,
        started_at_ms: Option<i64>,
        planned_end_at_ms: Option<i64>,
        paused: bool,
        paused_at_ms: Option<i64>,
        total_paused_ms: i64,
        template_id: Option<i64>,
    ) -> Self {
        Self(Arc::new(Mutex::new(FocusModeRuntime {
            status: FocusModeStatus {
                active,
                started_at_ms: active.then_some(started_at_ms).flatten(),
                planned_end_at_ms: active.then_some(planned_end_at_ms).flatten(),
                paused: active && paused,
                paused_at_ms: (active && paused).then_some(paused_at_ms).flatten(),
                total_paused_ms: if active { total_paused_ms } else { 0 },
                template_id: active.then_some(template_id).flatten(),
            },
            last_reminded_interval: 0,
        })))
    }

    pub fn snapshot(&self) -> FocusModeStatus {
        self.0
            .lock()
            .map(|runtime| runtime.status.clone())
            .unwrap_or_default()
    }

    pub fn replace(&self, status: FocusModeStatus) -> FocusModeStatus {
        let Ok(mut runtime) = self.0.lock() else {
            return FocusModeStatus::default();
        };
        runtime.status = status;
        runtime.last_reminded_interval = 0;
        runtime.status.clone()
    }

    pub fn start(
        &self,
        now_ms: i64,
        planned_end_at_ms: Option<i64>,
        template_id: Option<i64>,
    ) -> FocusModeStatus {
        let Ok(mut runtime) = self.0.lock() else {
            return FocusModeStatus::default();
        };
        runtime.status = FocusModeStatus {
            active: true,
            started_at_ms: Some(now_ms),
            planned_end_at_ms,
            paused: false,
            paused_at_ms: None,
            total_paused_ms: 0,
            template_id,
        };
        runtime.last_reminded_interval = 0;
        runtime.status.clone()
    }

    pub fn end(&self) -> FocusModeStatus {
        let Ok(mut runtime) = self.0.lock() else {
            return FocusModeStatus::default();
        };
        runtime.status = FocusModeStatus::default();
        runtime.last_reminded_interval = 0;
        runtime.status.clone()
    }

    pub fn set_paused(&self, paused: bool, now_ms: i64) -> FocusModeStatus {
        let Ok(mut runtime) = self.0.lock() else {
            return FocusModeStatus::default();
        };
        if !runtime.status.active || runtime.status.paused == paused {
            return runtime.status.clone();
        }
        if paused {
            runtime.status.paused = true;
            runtime.status.paused_at_ms = Some(now_ms);
        } else {
            let paused_duration = runtime
                .status
                .paused_at_ms
                .map(|started| now_ms.saturating_sub(started))
                .unwrap_or_default();
            runtime.status.total_paused_ms = runtime
                .status
                .total_paused_ms
                .saturating_add(paused_duration);
            runtime.status.planned_end_at_ms = runtime
                .status
                .planned_end_at_ms
                .map(|end| end.saturating_add(paused_duration));
            runtime.status.paused = false;
            runtime.status.paused_at_ms = None;
        }
        runtime.status.clone()
    }

    pub fn is_due(&self, now_ms: i64) -> bool {
        let status = self.snapshot();
        status.active && !status.paused && status.planned_end_at_ms.is_some_and(|end| now_ms >= end)
    }

    pub fn should_send_break_reminder(&self, now_ms: i64, interval_minutes: i64) -> bool {
        let Ok(mut runtime) = self.0.lock() else {
            return false;
        };
        let Some(started_at_ms) = runtime
            .status
            .started_at_ms
            .filter(|_| runtime.status.active && !runtime.status.paused)
        else {
            return false;
        };
        let interval_ms = interval_minutes.saturating_mul(60_000);
        if interval_ms <= 0 {
            return false;
        }
        let elapsed_ms = now_ms
            .saturating_sub(started_at_ms)
            .saturating_sub(runtime.status.total_paused_ms);
        let milestone = elapsed_ms / interval_ms;
        if milestone < 1 || milestone <= runtime.last_reminded_interval {
            return false;
        }
        runtime.last_reminded_interval = milestone;
        true
    }
}

pub fn is_quiet_hours(now: &str, start: &str, end: &str) -> bool {
    if start == end {
        return false;
    }
    if start < end {
        now >= start && now < end
    } else {
        now >= start || now < end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reminders_fire_once_per_elapsed_interval() {
        let state = FocusModeState::new(true, Some(1_000), None, false, None, 0, None);
        assert!(!state.should_send_break_reminder(60_999, 1));
        assert!(state.should_send_break_reminder(61_000, 1));
        assert!(!state.should_send_break_reminder(90_000, 1));
        assert!(state.should_send_break_reminder(121_000, 1));
    }

    #[test]
    fn resuming_extends_the_planned_end_by_the_pause_duration() {
        let state = FocusModeState::new(true, Some(1_000), Some(61_000), false, None, 0, None);
        state.set_paused(true, 11_000);
        let status = state.set_paused(false, 21_000);
        assert_eq!(status.planned_end_at_ms, Some(71_000));
        assert_eq!(status.total_paused_ms, 10_000);
        assert!(!state.should_send_break_reminder(61_000, 1));
        assert!(state.should_send_break_reminder(71_000, 1));
    }

    #[test]
    fn replacing_restored_focus_state_resets_runtime_reminders() {
        let state = FocusModeState::new(true, Some(1_000), None, false, None, 0, None);
        assert!(state.should_send_break_reminder(61_000, 1));
        let restored = FocusModeStatus {
            active: true,
            started_at_ms: Some(120_000),
            planned_end_at_ms: Some(240_000),
            ..FocusModeStatus::default()
        };

        let replaced = state.replace(restored);
        assert!(replaced.active);
        assert_eq!(replaced.started_at_ms, Some(120_000));
        assert_eq!(replaced.planned_end_at_ms, Some(240_000));
        assert!(!state.should_send_break_reminder(179_999, 1));
        assert!(state.should_send_break_reminder(180_000, 1));
    }

    #[test]
    fn quiet_hours_support_ranges_across_midnight() {
        assert!(is_quiet_hours("23:30", "22:00", "07:00"));
        assert!(is_quiet_hours("06:30", "22:00", "07:00"));
        assert!(!is_quiet_hours("12:00", "22:00", "07:00"));
        assert!(is_quiet_hours("12:00", "09:00", "17:00"));
    }
}
