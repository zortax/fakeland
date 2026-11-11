//! Constant frame rate pacing of a recording. Slot `i` of the video shows the screen at
//! `origin + i / rate`. When the screen updates at time `t`, every slot before `t` shows the
//! picture from before the update, so the pacer hands out those slots. Slots within an eighth of
//! a period before `t` count as at `t`, so a slot grid that equals the vblank grid never splits
//! on rounding.

use fakeland_types::{Nanos, Timestamp};

use super::FrameRate;

/// Hands out video slots as screen updates arrive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FramePacer {
    origin: Timestamp,
    rate: FrameRate,
    tolerance: Nanos,
    emitted: u64,
}

impl FramePacer {
    /// Starts a pacer whose first slot is at `origin`.
    pub fn new(origin: Timestamp, rate: FrameRate) -> Self {
        Self {
            origin,
            rate,
            tolerance: rate.period() / 8,
            emitted: 0,
        }
    }

    /// Returns the time of the first slot.
    pub fn origin(&self) -> Timestamp {
        self.origin
    }

    /// Returns the count of slots handed out so far.
    pub fn emitted(&self) -> u64 {
        self.emitted
    }

    /// Returns the time of slot `i`.
    #[cfg(test)]
    pub fn slot_time(&self, i: u64) -> Timestamp {
        self.origin + self.rate.offset(i)
    }

    /// Returns the count of new slots before an update at `t`, and counts them as handed out.
    pub fn take_before(&mut self, t: Timestamp) -> u64 {
        let due = self
            .rate
            .slots_before(t.since(self.origin) - self.tolerance);
        let n = due.saturating_sub(self.emitted);
        self.emitted = self.emitted.max(due);
        n
    }

    /// Returns the count of new slots up to and including `t`, for the end of a recording.
    pub fn take_through(&mut self, t: Timestamp) -> u64 {
        let due = self.rate.slots_before(t.since(self.origin) + Nanos(1));
        let n = due.saturating_sub(self.emitted);
        self.emitted = self.emitted.max(due);
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const P: i64 = 16_666_667;

    fn grid_pacer() -> FramePacer {
        FramePacer::new(
            Timestamp(1_000_000_000),
            FrameRate::from_period(Nanos(P)).unwrap(),
        )
    }

    #[test]
    fn one_slot_per_vblank_on_the_grid() {
        let mut p = grid_pacer();
        let vblank = |k: i64| Timestamp(1_000_000_000) + Nanos(P * k);
        assert_eq!(p.take_before(vblank(0)), 0);
        for k in 1..=120 {
            assert_eq!(p.take_before(vblank(k)), 1, "vblank {k}");
        }
        assert_eq!(p.emitted(), 120);
        assert_eq!(p.slot_time(120), vblank(120));
    }

    #[test]
    fn missed_updates_give_repeats_and_repeated_times_give_none() {
        let mut p = grid_pacer();
        let t = Timestamp(1_000_000_000) + Nanos(P * 5);
        assert_eq!(p.take_before(t), 5);
        assert_eq!(p.take_before(t), 0);
        // A jittered update one microsecond early still counts as the same vblank.
        assert_eq!(p.take_before(t + Nanos(P) - Nanos::from_micros(1)), 1);
    }

    #[test]
    fn half_rate_takes_every_other_vblank() {
        let mut p = FramePacer::new(Timestamp(0), FrameRate::new(30, 1).unwrap());
        let counts: Vec<u64> = (1..=8)
            .map(|k| p.take_before(Timestamp(0) + Nanos(P * k)))
            .collect();
        assert_eq!(counts.iter().sum::<u64>(), 4);
        assert!(counts.iter().all(|&c| c <= 1));
    }

    #[test]
    fn the_end_includes_the_last_slot() {
        let mut p = grid_pacer();
        let t = Timestamp(1_000_000_000) + Nanos(P * 3);
        assert_eq!(p.take_through(t), 4);
        assert_eq!(p.take_through(t), 0);
        assert_eq!(p.take_before(t + Nanos(P)), 0);
    }
}
