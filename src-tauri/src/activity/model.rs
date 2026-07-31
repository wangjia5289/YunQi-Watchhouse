use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ActivityState {
    Active,
    Idle,
}

impl ActivityState {
    pub(crate) const fn as_db_str(self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Idle => "IDLE",
        }
    }

    pub(crate) fn from_db_str(value: &str) -> rusqlite::Result<Self> {
        match value {
            "ACTIVE" => Ok(Self::Active),
            "IDLE" => Ok(Self::Idle),
            other => Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown activity state: {other}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ClosedReason {
    AppChanged,
    BecameIdle,
    BecameActive,
    Midnight,
    SleepGap,
    ClockChanged,
    Paused,
    Shutdown,
    CrashRecovery,
}

impl ClosedReason {
    pub(crate) const fn as_db_str(self) -> &'static str {
        match self {
            Self::AppChanged => "APP_CHANGED",
            Self::BecameIdle => "BECAME_IDLE",
            Self::BecameActive => "BECAME_ACTIVE",
            Self::Midnight => "MIDNIGHT",
            Self::SleepGap => "SLEEP_GAP",
            Self::ClockChanged => "CLOCK_CHANGED",
            Self::Paused => "PAUSED",
            Self::Shutdown => "SHUTDOWN",
            Self::CrashRecovery => "CRASH_RECOVERY",
        }
    }

    pub(crate) fn from_db_str(value: &str) -> rusqlite::Result<Self> {
        match value {
            "APP_CHANGED" => Ok(Self::AppChanged),
            "BECAME_IDLE" => Ok(Self::BecameIdle),
            "BECAME_ACTIVE" => Ok(Self::BecameActive),
            "MIDNIGHT" => Ok(Self::Midnight),
            "SLEEP_GAP" => Ok(Self::SleepGap),
            "CLOCK_CHANGED" => Ok(Self::ClockChanged),
            "PAUSED" => Ok(Self::Paused),
            "SHUTDOWN" => Ok(Self::Shutdown),
            "CRASH_RECOVERY" => Ok(Self::CrashRecovery),
            other => Err(rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                format!("unknown closed reason: {other}").into(),
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Application {
    pub id: i64,
    pub identity_key: String,
    pub name: String,
    pub bundle_id: Option<String>,
    pub executable_path: Option<String>,
    pub category: String,
    pub is_ignored: bool,
    pub record_window_titles: bool,
    pub first_seen_at_ms: i64,
    pub last_seen_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewApplication {
    pub name: String,
    pub bundle_id: Option<String>,
    pub executable_path: Option<String>,
    pub seen_at_ms: i64,
}

impl NewApplication {
    pub fn identity_key(&self) -> String {
        if let Some(bundle_id) = self.bundle_id.as_deref().filter(|value| !value.is_empty()) {
            format!("bundle:{bundle_id}")
        } else if let Some(path) = self
            .executable_path
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            format!("path:{path}")
        } else {
            format!("name:{}", self.name)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySession {
    pub id: i64,
    pub state: ActivityState,
    pub application_id: Option<i64>,
    pub window_title: Option<String>,
    pub note: Option<String>,
    pub started_at_ms: i64,
    pub ended_at_ms: i64,
    pub duration_ms: i64,
    pub is_open: bool,
    pub closed_reason: Option<ClosedReason>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSession {
    pub state: ActivityState,
    pub application_id: Option<i64>,
    pub window_title: Option<String>,
    pub category_override: Option<String>,
    pub started_at_ms: i64,
}
