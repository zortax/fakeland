//! Waiting until a deadline on the session clock, by spinning or by sleeping, and burning CPU
//! time with real work.

use std::hint::black_box;

use fakeland_types::{Clock, Nanos, SplitMix64, Timestamp};

use super::DelayKind;

/// Waits until the clock reaches `deadline`. Returns at once for a past deadline.
pub fn wait_until(clock: &dyn Clock, deadline: Timestamp, kind: DelayKind) {
    loop {
        let now = clock.now();
        if now >= deadline {
            return;
        }
        match kind {
            DelayKind::Spin => std::hint::spin_loop(),
            DelayKind::Sleep => std::thread::sleep(deadline.since(now).to_duration()),
        }
    }
}

/// Runs integer work on the CPU for `duration` and returns a checksum of the work.
pub fn burn(clock: &dyn Clock, duration: Nanos) -> u64 {
    if duration.0 <= 0 {
        return 0;
    }
    let deadline = clock.now() + duration;
    let mut rng = SplitMix64::new(0x5eed);
    let mut acc = 0u64;
    loop {
        for _ in 0..256 {
            acc = acc.rotate_left(5) ^ rng.next_u64();
        }
        acc = black_box(acc);
        if clock.now() >= deadline {
            return acc;
        }
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::MonotonicClock;

    use super::*;

    #[test]
    fn spin_reaches_the_deadline() {
        let clock = MonotonicClock;
        let start = clock.now();
        let deadline = start + Nanos::from_micros(500);
        wait_until(&clock, deadline, DelayKind::Spin);
        let end = clock.now();
        assert!(end >= deadline);
    }

    #[test]
    fn sleep_reaches_the_deadline() {
        let clock = MonotonicClock;
        let deadline = clock.now() + Nanos::from_millis(2);
        wait_until(&clock, deadline, DelayKind::Sleep);
        assert!(clock.now() >= deadline);
    }

    #[test]
    fn past_deadline_returns_at_once() {
        let clock = MonotonicClock;
        wait_until(&clock, Timestamp(0), DelayKind::Sleep);
    }

    #[test]
    fn burn_takes_its_time() {
        let clock = MonotonicClock;
        let start = clock.now();
        burn(&clock, Nanos::from_millis(2));
        assert!(clock.now().since(start) >= Nanos::from_millis(2));
        assert_eq!(burn(&clock, Nanos::ZERO), 0);
    }
}
