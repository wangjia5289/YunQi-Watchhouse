use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusModeStatus {
    pub active: bool,
    pub started_at_ms: Option<i64>,
}

#[derive(Default)]
struct FocusModeRuntime {
    status: FocusModeStatus,
    last_reminded_interval: i64,
}

#[derive(Clone, Default)]
pub struct FocusModeState(Arc<Mutex<FocusModeRuntime>>);

impl FocusModeState {
    pub fn new(active: bool, started_at_ms: Option<i64>) -> Self {
        Self(Arc::new(Mutex::new(FocusModeRuntime {
            status: FocusModeStatus {
                active,
                started_at_ms: active.then_some(started_at_ms).flatten(),
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

    pub fn set_active(&self, active: bool, now_ms: i64) -> FocusModeStatus {
        let Ok(mut runtime) = self.0.lock() else {
            return FocusModeStatus::default();
        };
        runtime.status.active = active;
        runtime.status.started_at_ms = active.then_some(now_ms);
        runtime.last_reminded_interval = 0;
        runtime.status.clone()
    }

    pub fn should_send_break_reminder(&self, now_ms: i64, interval_minutes: i64) -> bool {
        let Ok(mut runtime) = self.0.lock() else {
            return false;
        };
        let Some(started_at_ms) = runtime.status.started_at_ms.filter(|_| runtime.status.active)
        else {
            return false;
        };
        let interval_ms = interval_minutes.saturating_mul(60_000);
        if interval_ms <= 0 {
            return false;
        }
        let milestone = now_ms.saturating_sub(started_at_ms) / interval_ms;
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
        let state = FocusModeState::new(true, Some(1_000));
        assert!(!state.should_send_break_reminder(60_999, 1));
        assert!(state.should_send_break_reminder(61_000, 1));
        assert!(!state.should_send_break_reminder(90_000, 1));
        assert!(state.should_send_break_reminder(121_000, 1));
    }

    #[test]
    fn quiet_hours_support_ranges_across_midnight() {
        assert!(is_quiet_hours("23:30", "22:00", "07:00"));
        assert!(is_quiet_hours("06:30", "22:00", "07:00"));
        assert!(!is_quiet_hours("12:00", "22:00", "07:00"));
        assert!(is_quiet_hours("12:00", "09:00", "17:00"));
    }
}
