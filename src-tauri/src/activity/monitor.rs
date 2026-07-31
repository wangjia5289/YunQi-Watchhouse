use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use tauri::async_runtime::JoinHandle;
use tokio::{
    sync::{mpsc, watch},
    time::{MissedTickBehavior, interval},
};
use tokio_util::sync::CancellationToken;

use crate::{
    error::{AppError, AppResult},
    platform::{ActivityProvider, ForegroundApplication},
};

use super::{ActivityState, IdleDetector, IdleObservation};

pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_secs(1);
pub const DEFAULT_IDLE_THRESHOLD: Duration = Duration::from_secs(3 * 60);
const MIN_POLL_INTERVAL: Duration = Duration::from_millis(500);
const MAX_POLL_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy)]
pub struct MonitorConfig {
    pub poll_interval: Duration,
    pub idle_threshold: Duration,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            poll_interval: DEFAULT_POLL_INTERVAL,
            idle_threshold: DEFAULT_IDLE_THRESHOLD,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivitySample {
    pub observed_at_ms: i64,
    pub state: ActivityState,
    pub idle_duration_ms: u64,
    pub last_input_at_ms: i64,
    pub foreground_application: Option<ForegroundApplication>,
    pub continuity: SampleContinuity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SampleContinuity {
    Continuous,
    Gap {
        previous_observed_at_ms: i64,
        reason: SamplingGapReason,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SamplingGapReason {
    SleepOrSuspend,
    ClockChanged,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "status",
    content = "payload",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
pub enum MonitorStatus {
    Starting,
    Paused,
    Running(ActivitySample),
    Degraded { message: String },
    Stopped,
}

type WindowTitlePolicy = Arc<dyn Fn(&ForegroundApplication) -> bool + Send + Sync + 'static>;

pub struct ActivityMonitor<P> {
    detector: IdleDetector<P>,
    poll_interval: Duration,
    window_title_policy: Option<WindowTitlePolicy>,
}

impl<P: ActivityProvider> ActivityMonitor<P> {
    pub fn new(provider: P, config: MonitorConfig) -> AppResult<Self> {
        if !(MIN_POLL_INTERVAL..=MAX_POLL_INTERVAL).contains(&config.poll_interval) {
            return Err(AppError::InvalidMonitorConfiguration(format!(
                "poll interval must be between {} and {} milliseconds",
                MIN_POLL_INTERVAL.as_millis(),
                MAX_POLL_INTERVAL.as_millis()
            )));
        }

        Ok(Self {
            detector: IdleDetector::new(provider, config.idle_threshold)?,
            poll_interval: config.poll_interval,
            window_title_policy: None,
        })
    }

    pub fn with_window_title_policy(
        mut self,
        policy: impl Fn(&ForegroundApplication) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.window_title_policy = Some(Arc::new(policy));
        self
    }

    pub fn sample_once(&self, observed_at_ms: i64) -> AppResult<ActivitySample> {
        self.sample_once_with_continuity(observed_at_ms, SampleContinuity::Continuous)
    }

    fn sample_once_with_continuity(
        &self,
        observed_at_ms: i64,
        continuity: SampleContinuity,
    ) -> AppResult<ActivitySample> {
        match self.detector.observe(observed_at_ms)? {
            IdleObservation::Active { idle_duration_ms } => {
                let elapsed_ms = i64::try_from(idle_duration_ms).unwrap_or(i64::MAX);
                let mut foreground = self.detector.provider().foreground_application()?;
                if self
                    .window_title_policy
                    .as_ref()
                    .is_some_and(|policy| policy(&foreground))
                {
                    foreground.window_title = self
                        .detector
                        .provider()
                        .window_title(&foreground)?
                        .and_then(|title| redact_window_title(&title));
                }
                Ok(ActivitySample {
                    observed_at_ms,
                    state: ActivityState::Active,
                    idle_duration_ms,
                    last_input_at_ms: observed_at_ms.saturating_sub(elapsed_ms),
                    foreground_application: Some(foreground),
                    continuity,
                })
            }
            IdleObservation::Idle {
                idle_duration_ms,
                last_input_at_ms,
            } => Ok(ActivitySample {
                observed_at_ms,
                state: ActivityState::Idle,
                idle_duration_ms,
                last_input_at_ms,
                foreground_application: None,
                continuity,
            }),
        }
    }
}

fn redact_window_title(title: &str) -> Option<String> {
    let title = title.trim();
    if title.is_empty() {
        return None;
    }
    let mut words = Vec::new();
    for word in title.split_whitespace() {
        let lower = word.to_ascii_lowercase();
        let redacted = if word.contains('@') {
            "[email]".to_owned()
        } else if let Some(index) = word.find('?') {
            format!("{}?[redacted]", &word[..index])
        } else if ["token=", "key=", "password=", "secret="]
            .iter()
            .any(|marker| lower.contains(marker))
        {
            "[secret]".to_owned()
        } else {
            redact_long_digit_runs(word)
        };
        words.push(redacted);
    }
    let result = words.join(" ");
    Some(result.chars().take(240).collect())
}

fn redact_long_digit_runs(value: &str) -> String {
    let mut output = String::new();
    let mut digits = String::new();
    for character in value.chars().chain(std::iter::once('\0')) {
        if character.is_ascii_digit() {
            digits.push(character);
        } else {
            if digits.len() >= 6 {
                output.push_str("[number]");
            } else {
                output.push_str(&digits);
            }
            digits.clear();
            if character != '\0' {
                output.push(character);
            }
        }
    }
    output
}

impl<P: ActivityProvider + 'static> ActivityMonitor<P> {
    pub fn spawn(self) -> MonitorHandle {
        self.spawn_inner(None)
    }

    pub fn spawn_with_sample_sink(
        self,
        sample_sender: mpsc::UnboundedSender<ActivitySample>,
    ) -> MonitorHandle {
        self.spawn_inner(Some(sample_sender))
    }

    fn spawn_inner(
        mut self,
        sample_sender: Option<mpsc::UnboundedSender<ActivitySample>>,
    ) -> MonitorHandle {
        let cancellation = CancellationToken::new();
        let task_cancellation = cancellation.clone();
        let paused = Arc::new(AtomicBool::new(false));
        let task_paused = paused.clone();
        let idle_threshold_seconds = Arc::new(AtomicU64::new(self.detector.threshold().as_secs()));
        let task_idle_threshold_seconds = idle_threshold_seconds.clone();
        let (status_sender, status_receiver) = watch::channel(MonitorStatus::Starting);

        let task = tauri::async_runtime::spawn(async move {
            let mut ticker = interval(self.poll_interval);
            ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
            let mut previous_success: Option<(Instant, i64)> = None;

            loop {
                tokio::select! {
                    _ = task_cancellation.cancelled() => break,
                    _ = ticker.tick() => {
                        if task_paused.load(Ordering::Relaxed) {
                            let _ = status_sender.send(MonitorStatus::Paused);
                            continue;
                        }
                        let threshold = Duration::from_secs(
                            task_idle_threshold_seconds.load(Ordering::Relaxed)
                        );
                        if self.detector.threshold() != threshold
                            && let Err(error) = self.detector.set_threshold(threshold)
                        {
                            let _ = status_sender.send(MonitorStatus::Degraded {
                                message: error.to_string(),
                            });
                            continue;
                        }
                        let monotonic_now = Instant::now();
                        let status = match unix_timestamp_ms().and_then(|wall_now| {
                            let continuity = sample_continuity(
                                previous_success,
                                monotonic_now,
                                wall_now,
                                self.poll_interval,
                            );
                            self.sample_once_with_continuity(wall_now, continuity)
                                .inspect(|_| previous_success = Some((monotonic_now, wall_now)))
                        }) {
                            Ok(sample) => {
                                if let Some(sender) = &sample_sender {
                                    let _ = sender.send(sample.clone());
                                }
                                MonitorStatus::Running(sample)
                            }
                            Err(error) => {
                                log::warn!("activity monitor degraded: {error}");
                                MonitorStatus::Degraded {
                                    message: error.to_string(),
                                }
                            },
                        };

                        if status_sender.send(status).is_err() {
                            break;
                        }
                    }
                }
            }

            let _ = status_sender.send(MonitorStatus::Stopped);
        });

        MonitorHandle {
            status_receiver,
            cancellation,
            paused,
            idle_threshold_seconds,
            task: Mutex::new(Some(task)),
        }
    }
}

pub struct MonitorHandle {
    status_receiver: watch::Receiver<MonitorStatus>,
    cancellation: CancellationToken,
    paused: Arc<AtomicBool>,
    idle_threshold_seconds: Arc<AtomicU64>,
    task: Mutex<Option<JoinHandle<()>>>,
}

impl MonitorHandle {
    pub fn subscribe(&self) -> watch::Receiver<MonitorStatus> {
        self.status_receiver.clone()
    }

    pub fn current_status(&self) -> MonitorStatus {
        self.status_receiver.borrow().clone()
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::Relaxed);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    pub fn set_idle_threshold(&self, threshold: Duration) -> AppResult<()> {
        super::IdleDetector::new(ThresholdValidationProvider, threshold)?;
        self.idle_threshold_seconds
            .store(threshold.as_secs(), Ordering::Relaxed);
        Ok(())
    }

    pub async fn shutdown(&self) {
        self.cancellation.cancel();
        let task = self.task.lock().ok().and_then(|mut task| task.take());
        if let Some(task) = task {
            let _ = task.await;
        }
    }
}

#[derive(Clone, Copy)]
struct ThresholdValidationProvider;

impl crate::platform::IdleTimeProvider for ThresholdValidationProvider {
    fn idle_duration(&self) -> AppResult<Duration> {
        Ok(Duration::ZERO)
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        self.cancellation.cancel();
        if let Ok(mut task) = self.task.lock()
            && let Some(task) = task.take()
        {
            task.abort();
        }
    }
}

fn unix_timestamp_ms() -> AppResult<i64> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| AppError::InvalidSystemClock)?;
    i64::try_from(elapsed.as_millis()).map_err(|_| AppError::InvalidSystemClock)
}

fn sample_continuity(
    previous: Option<(Instant, i64)>,
    monotonic_now: Instant,
    wall_now_ms: i64,
    poll_interval: Duration,
) -> SampleContinuity {
    let Some((previous_monotonic, previous_wall_ms)) = previous else {
        return SampleContinuity::Continuous;
    };
    let monotonic_delta_ms =
        i64::try_from(monotonic_now.duration_since(previous_monotonic).as_millis())
            .unwrap_or(i64::MAX);
    let wall_delta_ms = wall_now_ms.saturating_sub(previous_wall_ms);
    let gap_limit_ms = i64::try_from((poll_interval * 5).max(Duration::from_secs(5)).as_millis())
        .unwrap_or(i64::MAX);

    let reason = if monotonic_delta_ms > gap_limit_ms {
        Some(SamplingGapReason::SleepOrSuspend)
    } else if wall_delta_ms.abs_diff(monotonic_delta_ms) > 5_000 {
        Some(SamplingGapReason::ClockChanged)
    } else {
        None
    };

    reason.map_or(SampleContinuity::Continuous, |reason| {
        SampleContinuity::Gap {
            previous_observed_at_ms: previous_wall_ms,
            reason,
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::platform::{ForegroundApplicationProvider, IdleTimeProvider};

    #[derive(Clone)]
    struct MockActivityProvider {
        idle_duration: Duration,
        foreground_queries: Arc<AtomicUsize>,
    }

    impl IdleTimeProvider for MockActivityProvider {
        fn idle_duration(&self) -> AppResult<Duration> {
            Ok(self.idle_duration)
        }
    }

    impl ForegroundApplicationProvider for MockActivityProvider {
        fn foreground_application(&self) -> AppResult<ForegroundApplication> {
            self.foreground_queries.fetch_add(1, Ordering::Relaxed);
            Ok(ForegroundApplication {
                name: "Terminal".to_owned(),
                bundle_identifier: Some("com.apple.Terminal".to_owned()),
                executable_path: None,
                process_identifier: None,
                window_title: None,
            })
        }
    }

    #[test]
    fn window_title_redaction_removes_common_sensitive_values() {
        assert_eq!(
            redact_window_title(
                "Inbox user@example.com https://example.test/path?token=abc Account 12345678"
            ),
            Some("Inbox [email] https://example.test/path?[redacted] Account [number]".to_owned())
        );
        assert_eq!(redact_window_title("   "), None);
    }

    fn monitor(
        idle_duration: Duration,
    ) -> (ActivityMonitor<MockActivityProvider>, Arc<AtomicUsize>) {
        let foreground_queries = Arc::new(AtomicUsize::new(0));
        let provider = MockActivityProvider {
            idle_duration,
            foreground_queries: foreground_queries.clone(),
        };
        (
            ActivityMonitor::new(provider, MonitorConfig::default())
                .expect("monitor configuration should be valid"),
            foreground_queries,
        )
    }

    #[test]
    fn active_sample_includes_foreground_application() {
        let (monitor, foreground_queries) = monitor(Duration::from_secs(2));
        let sample = monitor.sample_once(10_000).expect("sample should succeed");

        assert_eq!(sample.state, ActivityState::Active);
        assert_eq!(sample.last_input_at_ms, 8_000);
        assert_eq!(
            sample
                .foreground_application
                .expect("active sample should have an app")
                .name,
            "Terminal"
        );
        assert_eq!(foreground_queries.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn idle_sample_skips_foreground_application_query() {
        let (monitor, foreground_queries) = monitor(Duration::from_secs(240));
        let sample = monitor
            .sample_once(1_000_000)
            .expect("sample should succeed");

        assert_eq!(sample.state, ActivityState::Idle);
        assert_eq!(sample.last_input_at_ms, 760_000);
        assert!(sample.foreground_application.is_none());
        assert_eq!(foreground_queries.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn rejects_poll_interval_outside_low_overhead_bounds() {
        let provider = MockActivityProvider {
            idle_duration: Duration::ZERO,
            foreground_queries: Arc::new(AtomicUsize::new(0)),
        };

        assert!(matches!(
            ActivityMonitor::new(
                provider,
                MonitorConfig {
                    poll_interval: Duration::from_millis(100),
                    ..MonitorConfig::default()
                }
            ),
            Err(AppError::InvalidMonitorConfiguration(_))
        ));
    }

    #[test]
    fn detects_long_monotonic_sampling_gap() {
        let now = Instant::now();
        assert_eq!(
            sample_continuity(
                Some((now - Duration::from_secs(30), 1_000)),
                now,
                31_000,
                Duration::from_secs(1),
            ),
            SampleContinuity::Gap {
                previous_observed_at_ms: 1_000,
                reason: SamplingGapReason::SleepOrSuspend,
            }
        );
    }

    #[test]
    fn detects_wall_clock_jump_without_long_monotonic_gap() {
        let now = Instant::now();
        assert_eq!(
            sample_continuity(
                Some((now - Duration::from_secs(1), 1_000)),
                now,
                20_000,
                Duration::from_secs(1),
            ),
            SampleContinuity::Gap {
                previous_observed_at_ms: 1_000,
                reason: SamplingGapReason::ClockChanged,
            }
        );
    }
}
