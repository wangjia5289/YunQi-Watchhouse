use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, AppResult},
    platform::IdleTimeProvider,
};

pub const MIN_IDLE_THRESHOLD: Duration = Duration::from_secs(30);
pub const MAX_IDLE_THRESHOLD: Duration = Duration::from_secs(60 * 60);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdleObservation {
    Active {
        idle_duration_ms: u64,
    },
    Idle {
        idle_duration_ms: u64,
        last_input_at_ms: i64,
    },
}

pub struct IdleDetector<P> {
    provider: P,
    threshold: Duration,
}

impl<P: IdleTimeProvider> IdleDetector<P> {
    pub fn new(provider: P, threshold: Duration) -> AppResult<Self> {
        validate_threshold(threshold)?;
        Ok(Self {
            provider,
            threshold,
        })
    }

    pub const fn threshold(&self) -> Duration {
        self.threshold
    }

    pub const fn provider(&self) -> &P {
        &self.provider
    }

    pub fn set_threshold(&mut self, threshold: Duration) -> AppResult<()> {
        validate_threshold(threshold)?;
        self.threshold = threshold;
        Ok(())
    }

    pub fn observe(&self, now_utc_ms: i64) -> AppResult<IdleObservation> {
        let idle_duration = self.provider.idle_duration()?;
        let idle_duration_ms = duration_as_millis_u64(idle_duration);

        if idle_duration >= self.threshold {
            let elapsed_ms = i64::try_from(idle_duration_ms).unwrap_or(i64::MAX);
            Ok(IdleObservation::Idle {
                idle_duration_ms,
                last_input_at_ms: now_utc_ms.saturating_sub(elapsed_ms),
            })
        } else {
            Ok(IdleObservation::Active { idle_duration_ms })
        }
    }
}

fn validate_threshold(threshold: Duration) -> AppResult<()> {
    if !(MIN_IDLE_THRESHOLD..=MAX_IDLE_THRESHOLD).contains(&threshold) {
        return Err(AppError::InvalidIdleThreshold(format!(
            "must be between {} and {} seconds",
            MIN_IDLE_THRESHOLD.as_secs(),
            MAX_IDLE_THRESHOLD.as_secs()
        )));
    }
    Ok(())
}

fn duration_as_millis_u64(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct MockIdleTimeProvider {
        idle_duration: Duration,
    }

    impl IdleTimeProvider for MockIdleTimeProvider {
        fn idle_duration(&self) -> AppResult<Duration> {
            Ok(self.idle_duration)
        }
    }

    fn detector(idle_seconds: u64, threshold_seconds: u64) -> IdleDetector<MockIdleTimeProvider> {
        IdleDetector::new(
            MockIdleTimeProvider {
                idle_duration: Duration::from_secs(idle_seconds),
            },
            Duration::from_secs(threshold_seconds),
        )
        .expect("threshold should be valid")
    }

    #[test]
    fn remains_active_below_threshold() {
        assert_eq!(
            detector(179, 180)
                .observe(1_000_000)
                .expect("observation should succeed"),
            IdleObservation::Active {
                idle_duration_ms: 179_000
            }
        );
    }

    #[test]
    fn becomes_idle_at_threshold_and_backdates_last_input() {
        assert_eq!(
            detector(180, 180)
                .observe(1_000_000)
                .expect("observation should succeed"),
            IdleObservation::Idle {
                idle_duration_ms: 180_000,
                last_input_at_ms: 820_000,
            }
        );
    }

    #[test]
    fn last_input_calculation_saturates_on_extreme_clock_value() {
        assert_eq!(
            detector(180, 180)
                .observe(i64::MIN + 1)
                .expect("observation should succeed"),
            IdleObservation::Idle {
                idle_duration_ms: 180_000,
                last_input_at_ms: i64::MIN,
            }
        );
    }

    #[test]
    fn rejects_threshold_outside_supported_settings_range() {
        assert!(matches!(
            IdleDetector::new(
                MockIdleTimeProvider {
                    idle_duration: Duration::ZERO,
                },
                Duration::from_secs(29),
            ),
            Err(AppError::InvalidIdleThreshold(_))
        ));
        assert!(matches!(
            IdleDetector::new(
                MockIdleTimeProvider {
                    idle_duration: Duration::ZERO,
                },
                Duration::from_secs(3_601),
            ),
            Err(AppError::InvalidIdleThreshold(_))
        ));
    }

    #[test]
    fn threshold_can_be_updated_after_validation() {
        let mut detector = detector(120, 180);
        detector
            .set_threshold(Duration::from_secs(60))
            .expect("threshold should update");

        assert_eq!(detector.threshold(), Duration::from_secs(60));
        assert!(matches!(
            detector
                .observe(1_000_000)
                .expect("observation should succeed"),
            IdleObservation::Idle { .. }
        ));
    }
}
