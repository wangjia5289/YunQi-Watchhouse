use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::AppResult;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{MacOsActivityProvider, application_icon_png, application_icon_revision};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForegroundApplication {
    pub name: String,
    pub bundle_identifier: Option<String>,
    pub executable_path: Option<String>,
}

/// Small platform boundary for reading elapsed time since genuine user input.
///
/// Implementations return duration only. They never expose keys, mouse
/// positions, or any other input content.
pub trait IdleTimeProvider: Send + Sync {
    fn idle_duration(&self) -> AppResult<Duration>;
}

pub trait ForegroundApplicationProvider: Send + Sync {
    fn foreground_application(&self) -> AppResult<ForegroundApplication>;
}

pub trait ActivityProvider: IdleTimeProvider + ForegroundApplicationProvider {}

impl<T> ActivityProvider for T where T: IdleTimeProvider + ForegroundApplicationProvider {}
