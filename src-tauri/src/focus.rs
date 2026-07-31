use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusModeStatus {
    pub active: bool,
    pub started_at_ms: Option<i64>,
}

#[derive(Clone, Default)]
pub struct FocusModeState(Arc<Mutex<FocusModeStatus>>);

impl FocusModeState {
    pub fn snapshot(&self) -> FocusModeStatus {
        self.0
            .lock()
            .map(|status| status.clone())
            .unwrap_or_default()
    }

    pub fn set_active(&self, active: bool, now_ms: i64) -> FocusModeStatus {
        let Ok(mut status) = self.0.lock() else {
            return FocusModeStatus::default();
        };
        status.active = active;
        status.started_at_ms = active.then_some(now_ms);
        status.clone()
    }
}
