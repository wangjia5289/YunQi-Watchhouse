mod detector;
mod manager;
mod model;
mod monitor;

pub use detector::{IdleDetector, IdleObservation};
pub use manager::{
    SessionManager, SessionManagerConfig, SessionManagerHandle, SessionManagerStatus,
};
pub use model::{
    ActivitySession, ActivityState, Application, ClosedReason, NewApplication, NewSession,
};
pub use monitor::{
    ActivityMonitor, ActivitySample, MonitorConfig, MonitorHandle, MonitorStatus, SampleContinuity,
    SamplingGapReason,
};
