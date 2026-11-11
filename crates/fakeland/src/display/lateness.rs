//! Wake lateness bookkeeping. Each output wake adds the time from its earliest deadline to the
//! wake: vblanks, latches, applies and frame callback dispatches. A run whose lateness passes the
//! threshold is invalid.

use fakeland_types::{Nanos, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The validity of a run, from the compositor wake lateness.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunValidity {
    /// The count of output wakes.
    pub wakes: u64,
    /// The count of wakes later than the threshold.
    pub late_wakes: u64,
    /// The largest lateness seen.
    pub max_lateness: Nanos,
    /// The mean lateness.
    pub mean_lateness: Nanos,
    /// The threshold.
    pub threshold: Nanos,
    /// The first wake later than the threshold.
    pub first_late: Option<Timestamp>,
}

impl RunValidity {
    /// Returns true when no wake passed the threshold.
    pub fn is_valid(&self) -> bool {
        self.late_wakes == 0
    }
}

/// Accumulates output wake lateness.
#[derive(Clone, Debug)]
pub struct LatenessTracker {
    threshold: Nanos,
    wakes: u64,
    late: u64,
    max: Nanos,
    sum: i128,
    first_late: Option<Timestamp>,
}

impl LatenessTracker {
    /// Builds a tracker with a threshold.
    pub fn new(threshold: Nanos) -> Self {
        Self {
            threshold,
            wakes: 0,
            late: 0,
            max: Nanos::ZERO,
            sum: 0,
            first_late: None,
        }
    }

    /// Records one wake. Returns true when the wake passed the threshold.
    pub fn record(&mut self, ideal: Timestamp, woke: Timestamp) -> bool {
        let lateness = woke.since(ideal).max(Nanos::ZERO);
        self.wakes += 1;
        self.sum += lateness.as_nanos() as i128;
        self.max = self.max.max(lateness);
        let late = lateness > self.threshold;
        if late {
            self.late += 1;
            self.first_late.get_or_insert(woke);
        }
        late
    }

    /// Clears every count and keeps the threshold.
    pub fn reset(&mut self) {
        *self = Self::new(self.threshold);
    }

    /// Returns the validity so far.
    pub fn validity(&self) -> RunValidity {
        RunValidity {
            wakes: self.wakes,
            late_wakes: self.late,
            max_lateness: self.max,
            mean_lateness: if self.wakes == 0 {
                Nanos::ZERO
            } else {
                Nanos((self.sum / self.wakes as i128) as i64)
            },
            threshold: self.threshold,
            first_late: self.first_late,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_late_wakes() {
        let mut t = LatenessTracker::new(Nanos::from_micros(500));
        assert!(!t.record(Timestamp(1_000_000), Timestamp(1_000_100)));
        assert!(t.record(Timestamp(2_000_000), Timestamp(2_600_000)));
        assert!(!t.record(Timestamp(3_000_000), Timestamp(2_999_000)));
        let v = t.validity();
        assert_eq!(v.wakes, 3);
        assert_eq!(v.late_wakes, 1);
        assert_eq!(v.max_lateness, Nanos(600_000));
        assert_eq!(v.mean_lateness, Nanos(200_033));
        assert_eq!(v.first_late, Some(Timestamp(2_600_000)));
        assert!(!v.is_valid());
        t.reset();
        assert!(t.validity().is_valid());
        assert_eq!(t.validity().wakes, 0);
    }
}
