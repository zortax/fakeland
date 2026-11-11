//! A vblank grid: vblank `n` happens at `anchor + (n - anchor_seq) * period`. Scenario anchors and
//! latch deadlines read vblank times, sequence numbers and phases from it.

use fakeland_types::{Nanos, Timestamp};
use serde::{Deserialize, Serialize};

/// A vblank sequence number and its time.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GridPoint {
    /// The vblank sequence number.
    pub seq: u64,
    /// The ideal vblank time.
    pub time: Timestamp,
}

/// A regular vblank grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid {
    anchor: Timestamp,
    anchor_seq: u64,
    period: Nanos,
}

impl Grid {
    /// Builds a grid whose vblank `anchor_seq` happens at `anchor`. The period must be positive.
    pub fn new(anchor: Timestamp, anchor_seq: u64, period: Nanos) -> Self {
        Self {
            anchor,
            anchor_seq,
            period: Nanos(period.as_nanos().max(1)),
        }
    }

    /// Returns the refresh period.
    pub fn period(&self) -> Nanos {
        self.period
    }

    /// Returns the anchor vblank.
    pub fn anchor(&self) -> GridPoint {
        GridPoint {
            seq: self.anchor_seq,
            time: self.anchor,
        }
    }

    /// Returns the ideal time of a vblank.
    pub fn vblank_time(&self, seq: u64) -> Timestamp {
        let steps = seq as i128 - self.anchor_seq as i128;
        let offset = steps * self.period.as_nanos() as i128;
        let t = (self.anchor.as_nanos() as i128 + offset).clamp(0, u64::MAX as i128);
        Timestamp(t as u64)
    }

    /// Returns the sequence number of the last vblank at or before `t`.
    pub fn seq_at(&self, t: Timestamp) -> u64 {
        let steps = t
            .since(self.anchor)
            .as_nanos()
            .div_euclid(self.period.as_nanos());
        (self.anchor_seq as i128 + steps as i128).max(0) as u64
    }

    /// Returns the first vblank strictly after `t`.
    pub fn next_vblank_after(&self, t: Timestamp) -> GridPoint {
        let mut seq = self.seq_at(t) + 1;
        while self.vblank_time(seq) <= t {
            seq += 1;
        }
        GridPoint {
            seq,
            time: self.vblank_time(seq),
        }
    }

    /// Returns the span from the last vblank at or before `t` to `t`.
    pub fn phase_of(&self, t: Timestamp) -> Nanos {
        t - self.vblank_time(self.seq_at(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Grid {
        Grid::new(Timestamp(1_000_000), 10, Nanos(16_666_667))
    }

    #[test]
    fn vblank_times_follow_the_period() {
        let g = grid();
        assert_eq!(g.vblank_time(10), Timestamp(1_000_000));
        assert_eq!(g.vblank_time(13), Timestamp(1_000_000 + 3 * 16_666_667));
        assert_eq!(g.vblank_time(9), Timestamp(0));
        assert_eq!(g.vblank_time(0), Timestamp(0));
    }

    #[test]
    fn seq_and_phase() {
        let g = grid();
        let t = g.vblank_time(12) + Nanos(5);
        assert_eq!(g.seq_at(t), 12);
        assert_eq!(g.phase_of(t), Nanos(5));
        assert_eq!(g.seq_at(g.vblank_time(12)), 12);
        assert_eq!(g.phase_of(g.vblank_time(12)), Nanos::ZERO);
        assert_eq!(g.seq_at(g.vblank_time(12) - Nanos(1)), 11);
        assert_eq!(g.seq_at(Timestamp(0)), 9);
    }

    #[test]
    fn next_vblank_is_strictly_later() {
        let g = grid();
        let at = g.vblank_time(12);
        assert_eq!(g.next_vblank_after(at).seq, 13);
        assert_eq!(g.next_vblank_after(at - Nanos(1)).seq, 12);
        assert_eq!(g.next_vblank_after(at).time, g.vblank_time(13));
    }
}
