use std::{sync::Mutex, time::Duration};

use chrono::{Local, LocalResult, NaiveTime, TimeZone};
use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tokio::sync::{mpsc, watch};
use tokio_util::sync::CancellationToken;

use crate::{
    database::ActivityRepository,
    error::{AppError, AppResult},
    platform::ForegroundApplication,
};

use super::{
    ActivitySample, ActivitySession, ActivityState, Application, ClosedReason, NewApplication,
    NewSession, SampleContinuity, SamplingGapReason,
};

pub const DEFAULT_CHECKPOINT_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy)]
pub struct SessionManagerConfig {
    pub checkpoint_interval: Duration,
}

impl Default for SessionManagerConfig {
    fn default() -> Self {
        Self {
            checkpoint_interval: DEFAULT_CHECKPOINT_INTERVAL,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    content = "payload",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum SessionManagerStatus {
    Running,
    Degraded { message: String },
    Stopped,
}

struct ManagedSession {
    session: ActivitySession,
    application_identity: Option<String>,
    last_observed_at_ms: i64,
}

pub struct SessionManager {
    repository: ActivityRepository,
    current: Option<ManagedSession>,
    checkpoint_interval_ms: i64,
    suppress_until_ms: Option<i64>,
}

impl SessionManager {
    pub fn new(repository: ActivityRepository, config: SessionManagerConfig) -> AppResult<Self> {
        if config.checkpoint_interval < Duration::from_secs(5)
            || config.checkpoint_interval > Duration::from_secs(5 * 60)
        {
            return Err(AppError::InvalidMonitorConfiguration(
                "session checkpoint interval must be between 5 and 300 seconds".to_owned(),
            ));
        }
        if repository.open_session()?.is_some() {
            return Err(AppError::InvalidSession(
                "session manager requires crash recovery before startup".to_owned(),
            ));
        }

        Ok(Self {
            repository,
            current: None,
            checkpoint_interval_ms: i64::try_from(config.checkpoint_interval.as_millis())
                .unwrap_or(i64::MAX),
            suppress_until_ms: None,
        })
    }

    pub fn process_sample(&mut self, sample: ActivitySample) -> AppResult<()> {
        if let SampleContinuity::Gap {
            previous_observed_at_ms,
            reason,
        } = sample.continuity
        {
            return self.process_gap(sample, previous_observed_at_ms, reason);
        }

        if self
            .suppress_until_ms
            .is_some_and(|until| sample.observed_at_ms <= until)
        {
            return Ok(());
        }
        self.suppress_until_ms = None;

        if sample.state == ActivityState::Active
            && let Some(foreground) = sample.foreground_application.as_ref()
        {
            let candidate = NewApplication {
                name: foreground.name.clone(),
                bundle_id: foreground.bundle_identifier.clone(),
                executable_path: foreground.executable_path.clone(),
                seen_at_ms: sample.observed_at_ms,
            };
            if self
                .repository
                .is_application_ignored(&candidate.identity_key())?
            {
                return self.close_current_at(sample.observed_at_ms, ClosedReason::Paused);
            }
        }

        let midnight_boundary = self.current.as_ref().and_then(|current| {
            local_midnight_between(current.last_observed_at_ms, sample.observed_at_ms)
        });
        if let Some(boundary) = midnight_boundary
            && let Some(current) = self.current.take()
        {
            return match self.transition_at(&current, &sample, boundary, ClosedReason::Midnight) {
                Ok(next) => {
                    self.current = Some(next);
                    Ok(())
                }
                Err(error) => {
                    self.current = Some(current);
                    Err(error)
                }
            };
        }

        match self.current.take() {
            None => self.current = Some(self.start_from_sample(&sample, sample.observed_at_ms)?),
            Some(mut current) if session_matches_sample(&current, &sample) => {
                let result = self.checkpoint_if_due(&mut current, sample.observed_at_ms);
                current.last_observed_at_ms =
                    current.last_observed_at_ms.max(sample.observed_at_ms);
                self.current = Some(current);
                result?;
            }
            Some(current) => match self.transition(&current, &sample) {
                Ok(next) => self.current = Some(next),
                Err(error) => {
                    self.current = Some(current);
                    return Err(error);
                }
            },
        }
        Ok(())
    }

    pub fn current_session(&self) -> Option<&ActivitySession> {
        self.current.as_ref().map(|current| &current.session)
    }

    fn transition(
        &self,
        current: &ManagedSession,
        sample: &ActivitySample,
    ) -> AppResult<ManagedSession> {
        let (boundary_ms, reason) = match (current.session.state, sample.state) {
            (ActivityState::Active, ActivityState::Idle) => (
                sample.last_input_at_ms.max(current.session.started_at_ms),
                ClosedReason::BecameIdle,
            ),
            (ActivityState::Idle, ActivityState::Active) => {
                (sample.observed_at_ms, ClosedReason::BecameActive)
            }
            (ActivityState::Active, ActivityState::Active) => {
                (sample.observed_at_ms, ClosedReason::AppChanged)
            }
            (ActivityState::Idle, ActivityState::Idle) => {
                return Err(AppError::InvalidSession(
                    "matching IDLE sample unexpectedly requested a transition".to_owned(),
                ));
            }
        };

        self.transition_at(current, sample, boundary_ms, reason)
    }

    fn transition_at(
        &self,
        current: &ManagedSession,
        sample: &ActivitySample,
        boundary_ms: i64,
        reason: ClosedReason,
    ) -> AppResult<ManagedSession> {
        let next = self.prepare_new_session(sample, boundary_ms)?;
        let session = self.repository.transition_session(
            current.session.id,
            boundary_ms,
            reason,
            &next.session,
        )?;
        Ok(ManagedSession {
            session,
            application_identity: next.application_identity,
            last_observed_at_ms: sample.observed_at_ms,
        })
    }

    fn process_gap(
        &mut self,
        sample: ActivitySample,
        previous_observed_at_ms: i64,
        reason: SamplingGapReason,
    ) -> AppResult<()> {
        if let Some(current) = self.current.take() {
            let close_at_ms = previous_observed_at_ms
                .max(current.session.started_at_ms)
                .min(
                    current
                        .last_observed_at_ms
                        .max(current.session.started_at_ms),
                );
            let closed_reason = match reason {
                SamplingGapReason::SleepOrSuspend => ClosedReason::SleepGap,
                SamplingGapReason::ClockChanged => ClosedReason::ClockChanged,
            };
            if let Err(error) =
                self.repository
                    .close_session(current.session.id, close_at_ms, closed_reason)
            {
                self.current = Some(current);
                return Err(error);
            }
        }

        if reason == SamplingGapReason::ClockChanged
            && sample.observed_at_ms <= previous_observed_at_ms
        {
            self.suppress_until_ms = Some(previous_observed_at_ms);
            return Ok(());
        }

        self.current = Some(self.start_from_sample(&sample, sample.observed_at_ms)?);
        Ok(())
    }

    fn start_from_sample(
        &self,
        sample: &ActivitySample,
        started_at_ms: i64,
    ) -> AppResult<ManagedSession> {
        let next = self.prepare_new_session(sample, started_at_ms)?;
        let session = self.repository.create_session(&next.session)?;
        Ok(ManagedSession {
            session,
            application_identity: next.application_identity,
            last_observed_at_ms: sample.observed_at_ms,
        })
    }

    fn prepare_new_session(
        &self,
        sample: &ActivitySample,
        started_at_ms: i64,
    ) -> AppResult<PreparedSession> {
        match sample.state {
            ActivityState::Active => {
                let foreground = sample.foreground_application.as_ref().ok_or_else(|| {
                    AppError::InvalidSession(
                        "ACTIVE sample requires a foreground application".to_owned(),
                    )
                })?;
                let application = self.upsert_application(foreground, sample.observed_at_ms)?;
                Ok(PreparedSession {
                    session: NewSession {
                        state: ActivityState::Active,
                        application_id: Some(application.id),
                        window_title: foreground.window_title.clone(),
                        started_at_ms,
                    },
                    application_identity: Some(application.identity_key),
                })
            }
            ActivityState::Idle => Ok(PreparedSession {
                session: NewSession {
                    state: ActivityState::Idle,
                    application_id: None,
                    window_title: None,
                    started_at_ms,
                },
                application_identity: None,
            }),
        }
    }

    fn upsert_application(
        &self,
        foreground: &ForegroundApplication,
        seen_at_ms: i64,
    ) -> AppResult<Application> {
        self.repository.upsert_application(&NewApplication {
            name: foreground.name.clone(),
            bundle_id: foreground.bundle_identifier.clone(),
            executable_path: foreground.executable_path.clone(),
            seen_at_ms,
        })
    }

    fn checkpoint_if_due(
        &self,
        current: &mut ManagedSession,
        observed_at_ms: i64,
    ) -> AppResult<()> {
        if observed_at_ms.saturating_sub(current.session.updated_at_ms)
            >= self.checkpoint_interval_ms
        {
            current.session = self
                .repository
                .checkpoint_session(current.session.id, observed_at_ms)?;
        }
        Ok(())
    }

    fn close_current(&mut self, reason: ClosedReason) -> AppResult<()> {
        let Some(current) = self.current.take() else {
            return Ok(());
        };
        match self.repository.close_session(
            current.session.id,
            current
                .last_observed_at_ms
                .max(current.session.started_at_ms),
            reason,
        ) {
            Ok(_) => Ok(()),
            Err(error) => {
                self.current = Some(current);
                Err(error)
            }
        }
    }

    fn close_current_at(&mut self, ended_at_ms: i64, reason: ClosedReason) -> AppResult<()> {
        let Some(current) = self.current.take() else {
            return Ok(());
        };
        match self.repository.close_session(
            current.session.id,
            ended_at_ms.max(current.session.started_at_ms),
            reason,
        ) {
            Ok(_) => Ok(()),
            Err(error) => {
                self.current = Some(current);
                Err(error)
            }
        }
    }

    pub fn spawn(self) -> (SessionManagerHandle, mpsc::UnboundedSender<ActivitySample>) {
        self.spawn_with_notifier(|| {})
    }

    pub fn spawn_with_notifier<F>(
        mut self,
        notify_data_changed: F,
    ) -> (SessionManagerHandle, mpsc::UnboundedSender<ActivitySample>)
    where
        F: Fn() + Send + Sync + 'static,
    {
        let (sample_sender, mut sample_receiver) = mpsc::unbounded_channel();
        let (control_sender, mut control_receiver) = mpsc::unbounded_channel();
        let (status_sender, status_receiver) = watch::channel(SessionManagerStatus::Running);
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let task = tauri::async_runtime::spawn(async move {
            'worker: loop {
                tokio::select! {
                    _ = task_cancellation.cancelled() => {
                        while let Ok(sample) = sample_receiver.try_recv() {
                            if let Err(error) = self.process_sample(sample) {
                                let _ = status_sender.send(SessionManagerStatus::Degraded {
                                    message: error.to_string(),
                                });
                            }
                        }
                        break 'worker;
                    },
                    sample = sample_receiver.recv() => {
                        let Some(sample) = sample else { break };
                        let before = self.current_session().map(session_revision);
                        let status = match self.process_sample(sample) {
                            Ok(()) => {
                                let after = self.current_session().map(session_revision);
                                if before != after {
                                    notify_data_changed();
                                }
                                SessionManagerStatus::Running
                            }
                            Err(error) => SessionManagerStatus::Degraded {
                                // Errors contain technical persistence state,
                                // never input content or window titles.
                                message: {
                                    log::error!("session persistence degraded: {error}");
                                    error.to_string()
                                },
                            },
                        };
                        let _ = status_sender.send(status);
                    },
                    control = control_receiver.recv() => {
                        match control {
                            Some(ManagerControl::Pause(acknowledge)) => {
                                let status = match self.close_current(ClosedReason::Paused) {
                                    Ok(()) => {
                                        notify_data_changed();
                                        SessionManagerStatus::Running
                                    }
                                    Err(error) => SessionManagerStatus::Degraded {
                                        message: error.to_string(),
                                    },
                                };
                                let _ = status_sender.send(status);
                                let _ = acknowledge.send(());
                            }
                            None => break,
                        }
                    }
                }
            }
            let final_status = match self.close_current(ClosedReason::Shutdown) {
                Ok(()) => SessionManagerStatus::Stopped,
                Err(error) => SessionManagerStatus::Degraded {
                    message: error.to_string(),
                },
            };
            let _ = status_sender.send(final_status);
        });

        (
            SessionManagerHandle {
                status_receiver,
                cancellation,
                control_sender,
                task: Mutex::new(Some(task)),
            },
            sample_sender,
        )
    }
}

fn session_revision(session: &ActivitySession) -> (i64, i64, i64, bool) {
    (
        session.id,
        session.updated_at_ms,
        session.ended_at_ms,
        session.is_open,
    )
}

struct PreparedSession {
    session: NewSession,
    application_identity: Option<String>,
}

fn session_matches_sample(current: &ManagedSession, sample: &ActivitySample) -> bool {
    match (current.session.state, sample.state) {
        (ActivityState::Idle, ActivityState::Idle) => true,
        (ActivityState::Active, ActivityState::Active) => {
            let same_application = sample
                .foreground_application
                .as_ref()
                .map(application_identity)
                .as_ref()
                == current.application_identity.as_ref();
            let same_title = sample
                .foreground_application
                .as_ref()
                .and_then(|application| application.window_title.as_ref())
                == current.session.window_title.as_ref();
            same_application && same_title
        }
        _ => false,
    }
}

fn application_identity(application: &ForegroundApplication) -> String {
    if let Some(bundle_identifier) = application
        .bundle_identifier
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        format!("bundle:{bundle_identifier}")
    } else if let Some(path) = application
        .executable_path
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        format!("path:{path}")
    } else {
        format!("name:{}", application.name)
    }
}

fn local_midnight_between(previous_ms: i64, current_ms: i64) -> Option<i64> {
    midnight_between_in_timezone(&Local, previous_ms, current_ms)
}

fn midnight_between_in_timezone<T: TimeZone>(
    timezone: &T,
    previous_ms: i64,
    current_ms: i64,
) -> Option<i64> {
    if current_ms <= previous_ms {
        return None;
    }
    let previous = timezone.timestamp_millis_opt(previous_ms).single()?;
    let current = timezone.timestamp_millis_opt(current_ms).single()?;
    if previous.date_naive() == current.date_naive() {
        return None;
    }

    let midnight = current.date_naive().and_time(NaiveTime::MIN);
    match timezone.from_local_datetime(&midnight) {
        LocalResult::Single(value) => Some(value.timestamp_millis()),
        LocalResult::Ambiguous(earliest, _) => Some(earliest.timestamp_millis()),
        LocalResult::None => None,
    }
}

pub struct SessionManagerHandle {
    status_receiver: watch::Receiver<SessionManagerStatus>,
    cancellation: CancellationToken,
    control_sender: mpsc::UnboundedSender<ManagerControl>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl SessionManagerHandle {
    pub fn current_status(&self) -> SessionManagerStatus {
        self.status_receiver.borrow().clone()
    }

    pub async fn shutdown(&self) {
        self.cancellation.cancel();
        let task = self.task.lock().ok().and_then(|mut task| task.take());
        if let Some(task) = task {
            let _ = task.await;
        }
    }

    pub async fn pause(&self) {
        if let Some(receiver) = self.request_pause() {
            let _ = receiver.await;
        }
    }

    pub fn request_pause(&self) -> Option<tokio::sync::oneshot::Receiver<()>> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        if self
            .control_sender
            .send(ManagerControl::Pause(sender))
            .is_ok()
        {
            Some(receiver)
        } else {
            None
        }
    }
}

enum ManagerControl {
    Pause(tokio::sync::oneshot::Sender<()>),
}

impl Drop for SessionManagerHandle {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Ok(mut task) = self.task.lock()
            && let Some(task) = task.take()
        {
            task.abort();
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, FixedOffset};

    use crate::database::Database;

    use super::*;

    fn setup() -> (SessionManager, ActivityRepository) {
        let repository =
            ActivityRepository::new(Database::in_memory().expect("database should open"));
        let manager = SessionManager::new(repository.clone(), SessionManagerConfig::default())
            .expect("manager should initialize");
        (manager, repository)
    }

    fn active_sample(at_ms: i64, name: &str, bundle: &str) -> ActivitySample {
        ActivitySample {
            observed_at_ms: at_ms,
            state: ActivityState::Active,
            idle_duration_ms: 0,
            last_input_at_ms: at_ms,
            foreground_application: Some(ForegroundApplication {
                name: name.to_owned(),
                bundle_identifier: Some(bundle.to_owned()),
                executable_path: None,
                process_identifier: None,
                window_title: None,
            }),
            continuity: SampleContinuity::Continuous,
        }
    }

    fn idle_sample(observed_at_ms: i64, last_input_at_ms: i64) -> ActivitySample {
        ActivitySample {
            observed_at_ms,
            state: ActivityState::Idle,
            idle_duration_ms: observed_at_ms.saturating_sub(last_input_at_ms) as u64,
            last_input_at_ms,
            foreground_application: None,
            continuity: SampleContinuity::Continuous,
        }
    }

    #[test]
    fn consecutive_same_application_uses_one_session() {
        let (mut manager, repository) = setup();
        manager
            .process_sample(active_sample(100_000, "IDEA", "com.jetbrains.intellij"))
            .expect("first sample should succeed");
        manager
            .process_sample(active_sample(130_000, "IDEA", "com.jetbrains.intellij"))
            .expect("second sample should succeed");

        let open = repository
            .open_session()
            .expect("query should succeed")
            .expect("session should remain open");
        assert_eq!((open.started_at_ms, open.ended_at_ms), (100_000, 130_000));
        assert_eq!(open.duration_ms, 30_000);
    }

    #[test]
    fn application_change_closes_and_starts_sessions() {
        let (mut manager, repository) = setup();
        manager
            .process_sample(active_sample(100_000, "IDEA", "com.jetbrains.intellij"))
            .expect("IDEA sample should succeed");
        manager
            .process_sample(active_sample(130_000, "Safari", "com.apple.Safari"))
            .expect("Safari sample should succeed");

        let sessions = repository
            .sessions_overlapping(99_999, 130_001)
            .expect("query should succeed");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].closed_reason, Some(ClosedReason::AppChanged));
        assert_ne!(sessions[0].application_id, sessions[1].application_id);
    }

    #[test]
    fn ignored_application_closes_current_session_without_recording_a_replacement() {
        let (mut manager, repository) = setup();
        manager
            .process_sample(active_sample(100_000, "IDEA", "com.jetbrains.intellij"))
            .expect("first sample should succeed");
        let ignored = repository
            .upsert_application(&NewApplication {
                name: "Private".to_owned(),
                bundle_id: Some("example.private".to_owned()),
                executable_path: None,
                seen_at_ms: 110_000,
            })
            .expect("ignored application should be stored");
        repository
            .update_application_preferences(ignored.id, "Personal", true, false)
            .expect("application should be ignored");

        manager
            .process_sample(active_sample(130_000, "Private", "example.private"))
            .expect("ignored sample should succeed");

        assert!(
            repository
                .open_session()
                .expect("query should succeed")
                .is_none()
        );
        let sessions = repository
            .sessions_overlapping(99_999, 130_001)
            .expect("query should succeed");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].ended_at_ms, 130_000);
        assert_eq!(sessions[0].closed_reason, Some(ClosedReason::Paused));
    }

    #[test]
    fn active_to_idle_backdates_boundary_to_last_input() {
        let (mut manager, repository) = setup();
        manager
            .process_sample(active_sample(100_000, "IDEA", "com.jetbrains.intellij"))
            .expect("active sample should succeed");
        manager
            .process_sample(idle_sample(400_000, 220_000))
            .expect("idle sample should succeed");

        let sessions = repository
            .sessions_overlapping(99_999, 400_001)
            .expect("query should succeed");
        assert_eq!(sessions.len(), 2);
        assert_eq!(
            (sessions[0].ended_at_ms, sessions[0].duration_ms),
            (220_000, 120_000)
        );
        assert_eq!(sessions[0].closed_reason, Some(ClosedReason::BecameIdle));
        assert_eq!(
            (sessions[1].started_at_ms, sessions[1].state),
            (220_000, ActivityState::Idle)
        );
    }

    #[test]
    fn idle_to_active_starts_application_at_observation_time() {
        let (mut manager, repository) = setup();
        manager
            .process_sample(idle_sample(400_000, 220_000))
            .expect("idle sample should succeed");
        manager
            .process_sample(active_sample(410_000, "Safari", "com.apple.Safari"))
            .expect("active sample should succeed");

        let sessions = repository
            .sessions_overlapping(399_999, 410_001)
            .expect("query should succeed");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].closed_reason, Some(ClosedReason::BecameActive));
        assert_eq!(
            (sessions[1].started_at_ms, sessions[1].state),
            (410_000, ActivityState::Active)
        );
    }

    #[test]
    fn failed_transition_keeps_current_session_in_memory_and_database() {
        let (mut manager, repository) = setup();
        manager
            .process_sample(active_sample(100_000, "IDEA", "com.jetbrains.intellij"))
            .expect("first sample should succeed");
        let session_id = manager
            .current_session()
            .expect("current session should exist")
            .id;

        let invalid_sample = ActivitySample {
            observed_at_ms: 120_000,
            state: ActivityState::Active,
            idle_duration_ms: 0,
            last_input_at_ms: 120_000,
            foreground_application: None,
            continuity: SampleContinuity::Continuous,
        };
        assert!(manager.process_sample(invalid_sample).is_err());

        assert_eq!(
            manager
                .current_session()
                .expect("current session should be preserved")
                .id,
            session_id
        );
        assert_eq!(
            repository
                .open_session()
                .expect("query should succeed")
                .expect("database session should remain open")
                .id,
            session_id
        );
    }

    #[test]
    fn shutdown_closes_at_last_observed_time_not_last_checkpoint() {
        let (mut manager, repository) = setup();
        manager
            .process_sample(active_sample(100_000, "IDEA", "com.jetbrains.intellij"))
            .expect("first sample should succeed");
        manager
            .process_sample(active_sample(102_000, "IDEA", "com.jetbrains.intellij"))
            .expect("second sample should succeed");
        manager
            .close_current(ClosedReason::Shutdown)
            .expect("shutdown close should succeed");

        assert!(
            repository
                .open_session()
                .expect("query should succeed")
                .is_none()
        );
        let session = repository
            .sessions_overlapping(99_999, 102_001)
            .expect("query should succeed")
            .pop()
            .expect("closed session should exist");
        assert_eq!(session.ended_at_ms, 102_000);
        assert_eq!(session.closed_reason, Some(ClosedReason::Shutdown));
    }

    #[test]
    fn sleep_gap_does_not_assign_unknown_time_to_either_session() {
        let (mut manager, repository) = setup();
        manager
            .process_sample(active_sample(100_000, "IDEA", "com.jetbrains.intellij"))
            .expect("first sample should succeed");
        manager
            .process_sample(active_sample(120_000, "IDEA", "com.jetbrains.intellij"))
            .expect("checkpoint sample should succeed");
        let mut after_wake = active_sample(10_000_000, "IDEA", "com.jetbrains.intellij");
        after_wake.continuity = SampleContinuity::Gap {
            previous_observed_at_ms: 120_000,
            reason: SamplingGapReason::SleepOrSuspend,
        };
        manager
            .process_sample(after_wake)
            .expect("wake sample should succeed");

        let old = repository
            .sessions_overlapping(99_999, 120_001)
            .expect("query should succeed")
            .pop()
            .expect("old session should exist");
        assert_eq!(old.ended_at_ms, 120_000);
        assert_eq!(old.closed_reason, Some(ClosedReason::SleepGap));
        assert_eq!(
            repository
                .open_session()
                .expect("query should succeed")
                .expect("new session should exist")
                .started_at_ms,
            10_000_000
        );
    }

    #[test]
    fn backward_clock_change_suppresses_overlapping_timestamps() {
        let (mut manager, repository) = setup();
        manager
            .process_sample(active_sample(100_000, "IDEA", "com.jetbrains.intellij"))
            .expect("first sample should succeed");
        manager
            .process_sample(active_sample(120_000, "IDEA", "com.jetbrains.intellij"))
            .expect("second sample should succeed");

        let mut jumped_back = active_sample(50_000, "IDEA", "com.jetbrains.intellij");
        jumped_back.continuity = SampleContinuity::Gap {
            previous_observed_at_ms: 120_000,
            reason: SamplingGapReason::ClockChanged,
        };
        manager
            .process_sample(jumped_back)
            .expect("clock change should close safely");
        manager
            .process_sample(active_sample(60_000, "IDEA", "com.jetbrains.intellij"))
            .expect("overlapping time should be ignored");
        assert!(
            repository
                .open_session()
                .expect("query should succeed")
                .is_none()
        );

        manager
            .process_sample(active_sample(121_000, "IDEA", "com.jetbrains.intellij"))
            .expect("tracking should resume after clock catches up");
        assert_eq!(
            repository
                .open_session()
                .expect("query should succeed")
                .expect("session should resume")
                .started_at_ms,
            121_000
        );
    }

    #[test]
    fn continuous_samples_split_at_local_midnight() {
        let (mut manager, repository) = setup();
        let today = Local::now().date_naive();
        let tomorrow = today.succ_opt().expect("next date should exist");
        let before = Local
            .from_local_datetime(&today.and_hms_opt(23, 59, 59).expect("time should be valid"))
            .single()
            .expect("local time should be unambiguous")
            .timestamp_millis();
        let after = Local
            .from_local_datetime(&tomorrow.and_hms_opt(0, 0, 1).expect("time should be valid"))
            .single()
            .expect("local time should be unambiguous")
            .timestamp_millis();

        manager
            .process_sample(active_sample(before, "IDEA", "com.jetbrains.intellij"))
            .expect("first sample should succeed");
        manager
            .process_sample(active_sample(after, "IDEA", "com.jetbrains.intellij"))
            .expect("midnight sample should succeed");

        let sessions = repository
            .sessions_overlapping(before - 1, after + 1)
            .expect("query should succeed");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].closed_reason, Some(ClosedReason::Midnight));
        assert_eq!(sessions[0].ended_at_ms, sessions[1].started_at_ms);
    }

    #[test]
    fn midnight_boundary_respects_explicit_timezone() {
        let timezone = FixedOffset::east_opt(8 * 60 * 60).expect("offset should be valid");
        let previous = timezone
            .with_ymd_and_hms(2026, 7, 30, 23, 59, 59)
            .single()
            .expect("time should exist")
            .timestamp_millis();
        let current = timezone
            .with_ymd_and_hms(2026, 7, 31, 0, 0, 1)
            .single()
            .expect("time should exist")
            .timestamp_millis();
        let boundary = midnight_between_in_timezone(&timezone, previous, current)
            .expect("midnight should be detected");
        let boundary_local = timezone
            .timestamp_millis_opt(boundary)
            .single()
            .expect("boundary should convert");

        assert_eq!(boundary_local.day(), 31);
        assert_eq!(boundary_local.time(), NaiveTime::MIN);
    }
}
