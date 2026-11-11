//! A frame rate meter over a sliding one-second window of presentation times.

use std::collections::VecDeque;

use fakeland_types::{Nanos, Timestamp};

/// The span of the meter.
const WINDOW: Nanos = Nanos::from_secs(1);

/// Counts presentations in the last second.
#[derive(Debug, Default)]
pub struct FpsMeter {
    times: VecDeque<Timestamp>,
}

impl FpsMeter {
    /// Records a presentation.
    pub fn record(&mut self, t: Timestamp) {
        self.times.push_back(t);
        self.trim(t);
    }

    /// Drops every presentation.
    pub fn clear(&mut self) {
        self.times.clear();
    }

    /// Returns the frame rate at `now`: the intervals between the presentations of the last
    /// second over the span from the first of them to `now`, so the rate decays while idle.
    pub fn fps(&mut self, now: Timestamp) -> f64 {
        self.trim(now);
        let n = self.times.len();
        if n < 2 {
            return 0.0;
        }
        let first = self.times[0];
        let span = now.since(first).max(self.times[n - 1].since(first));
        (n - 1) as f64 / span.as_secs_f64()
    }

    fn trim(&mut self, now: Timestamp) {
        while let Some(&t) = self.times.front() {
            if now.since(t) > WINDOW {
                self.times.pop_front();
            } else {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measures_a_steady_rate() {
        let mut m = FpsMeter::default();
        let t0 = Timestamp(10_000_000_000);
        for i in 0..240 {
            m.record(t0 + Nanos(16_666_667 * i));
        }
        let fps = m.fps(t0 + Nanos(16_666_667 * 239));
        assert!((fps - 60.0).abs() < 1.0, "{fps}");
    }

    #[test]
    fn short_history_and_idle_decay() {
        let mut m = FpsMeter::default();
        let t0 = Timestamp(10_000_000_000);
        assert_eq!(m.fps(t0), 0.0);
        m.record(t0);
        m.record(t0 + Nanos::from_millis(100));
        assert!((m.fps(t0 + Nanos::from_millis(100)) - 10.0).abs() < 1e-9);
        assert_eq!(m.fps(t0 + Nanos::from_secs(5)), 0.0);
        m.record(t0 + Nanos::from_secs(6));
        m.clear();
        assert_eq!(m.fps(t0 + Nanos::from_secs(6)), 0.0);
    }
}
