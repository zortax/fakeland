//! The session clock. Every timestamp in fakeland comes from a `Clock`. `MonotonicClock` reads
//! `CLOCK_MONOTONIC`; `FakeClock` is set by tests.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Nanos, Timestamp};

/// A source of monotonic timestamps.
pub trait Clock: Send + Sync + fmt::Debug {
    /// Returns the current time.
    fn now(&self) -> Timestamp;
}

/// A shared clock handle.
pub type SharedClock = Arc<dyn Clock>;

/// The system `CLOCK_MONOTONIC`, the clock that `wp_presentation` advertises to clients.
#[derive(Clone, Copy, Debug, Default)]
pub struct MonotonicClock;

impl Clock for MonotonicClock {
    #[allow(clippy::disallowed_methods)]
    fn now(&self) -> Timestamp {
        let ts = rustix::time::clock_gettime(rustix::time::ClockId::Monotonic);
        Timestamp::from_secs_nanos(ts.tv_sec as u64, ts.tv_nsec as u32)
    }
}

/// A clock that moves only when a test moves it.
#[derive(Clone, Debug, Default)]
pub struct FakeClock {
    now: Arc<AtomicU64>,
}

impl FakeClock {
    /// Builds a fake clock at a start time.
    pub fn new(start: Timestamp) -> Self {
        Self {
            now: Arc::new(AtomicU64::new(start.0)),
        }
    }

    /// Sets the current time.
    pub fn set(&self, t: Timestamp) {
        self.now.store(t.0, Ordering::SeqCst);
    }

    /// Moves the clock forward by a span.
    pub fn advance(&self, by: Nanos) {
        let now = Timestamp(self.now.load(Ordering::SeqCst)) + by;
        self.set(now);
    }
}

impl Clock for FakeClock {
    fn now(&self) -> Timestamp {
        Timestamp(self.now.load(Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_clock_moves_forward() {
        let c = MonotonicClock;
        let a = c.now();
        let b = c.now();
        assert!(b >= a);
        assert!(a.0 > 0);
    }

    #[test]
    fn fake_clock_advances() {
        let c = FakeClock::new(Timestamp(100));
        c.advance(Nanos(50));
        assert_eq!(c.now(), Timestamp(150));
    }
}
