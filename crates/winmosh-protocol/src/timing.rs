use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetransmissionTimeout(pub Duration);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RttEstimator {
    srtt_ms: f64,
    rttvar_ms: f64,
    initialized: bool,
}

impl Default for RttEstimator {
    fn default() -> Self {
        Self {
            srtt_ms: 1000.0,
            rttvar_ms: 500.0,
            initialized: false,
        }
    }
}

impl RttEstimator {
    pub fn observe(&mut self, sample: Duration) {
        let sample_ms = sample.as_secs_f64() * 1000.0;
        if sample_ms >= 5000.0 {
            return;
        }
        if !self.initialized {
            self.srtt_ms = sample_ms;
            self.rttvar_ms = sample_ms / 2.0;
            self.initialized = true;
            return;
        }
        self.rttvar_ms = 0.75 * self.rttvar_ms + 0.25 * (self.srtt_ms - sample_ms).abs();
        self.srtt_ms = 0.875 * self.srtt_ms + 0.125 * sample_ms;
    }

    pub fn timeout(self) -> RetransmissionTimeout {
        let timeout_ms = (self.srtt_ms + 4.0 * self.rttvar_ms).clamp(50.0, 1000.0);
        RetransmissionTimeout(Duration::from_millis(timeout_ms.round() as u64))
    }

    pub fn srtt(&self) -> Option<Duration> {
        self.initialized
            .then(|| Duration::from_millis(self.srtt_ms.max(0.0).round() as u64))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TimestampClock {
    origin: Instant,
}

impl TimestampClock {
    pub fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }

    pub fn timestamp(&self) -> u64 {
        self.origin.elapsed().as_millis() as u64
    }

    pub fn timestamp16(&self) -> u16 {
        timestamp16_from(self.timestamp())
    }
}

impl Default for TimestampClock {
    fn default() -> Self {
        Self::new()
    }
}

pub fn timestamp16_from(timestamp: u64) -> u16 {
    let value = (timestamp % 65_536) as u16;
    if value == u16::MAX {
        0
    } else {
        value
    }
}

pub fn timestamp_diff(newer: u16, older: u16) -> u16 {
    newer.wrapping_sub(older)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{timestamp16_from, timestamp_diff, RttEstimator};

    #[test]
    fn timestamp_wraps_without_reserved_value() {
        assert_eq!(timestamp16_from(65_535), 0);
        assert_eq!(timestamp16_from(65_536), 0);
        assert_eq!(timestamp_diff(2, 65_534), 4);
    }

    #[test]
    fn rtt_estimator_clamps_timeout() {
        let mut estimator = RttEstimator::default();
        estimator.observe(Duration::from_millis(10));
        assert_eq!(estimator.timeout().0, Duration::from_millis(50));
        estimator.observe(Duration::from_millis(4000));
        assert!(estimator.timeout().0 <= Duration::from_millis(1000));
    }
}
