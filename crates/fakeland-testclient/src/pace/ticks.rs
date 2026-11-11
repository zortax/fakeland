//! A fixed tick grid `anchor + round(k * 1e9 / rate)` ns for the fixed-rate present mode. Each
//! tick time comes from its index, so fractional rates never drift. Ticks that pass while the
//! client is busy are skipped.

use fakeland_types::{Hz, Nanos, Timestamp};

/// The tick grid and the next tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ticks {
    anchor: Timestamp,
    period_ns: f64,
    index: u64,
}

impl Ticks {
    /// Builds a grid at a rate whose first tick is at `anchor`.
    ///
    /// # Panics
    /// Panics when the rate is not positive and finite.
    pub fn new(anchor: Timestamp, rate: Hz) -> Self {
        assert!(
            rate.0 > 0.0 && rate.0.is_finite(),
            "tick rate must be positive"
        );
        Self {
            anchor,
            period_ns: 1e9 / rate.0,
            index: 0,
        }
    }

    /// Returns the time of the next tick.
    pub fn next(&self) -> Timestamp {
        self.at(self.index)
    }

    /// Returns the index of the next tick.
    #[cfg(test)]
    pub fn index(&self) -> u64 {
        self.index
    }

    /// Returns true when the next tick is due.
    pub fn due(&self, now: Timestamp) -> bool {
        now >= self.next()
    }

    /// Moves past every tick at or before `now`. Returns the number of ticks that passed
    /// without use, beyond the one being consumed.
    pub fn advance(&mut self, now: Timestamp) -> u64 {
        if now < self.next() {
            return 0;
        }
        let elapsed = now.since(self.anchor).0 as f64;
        let mut last = ((elapsed / self.period_ns).floor() as u64).max(self.index);
        while self.at(last + 1) <= now {
            last += 1;
        }
        while last > self.index && self.at(last) > now {
            last -= 1;
        }
        let skipped = last - self.index;
        self.index = last + 1;
        skipped
    }

    fn at(&self, index: u64) -> Timestamp {
        self.anchor + Nanos((index as f64 * self.period_ns).round() as i64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_follow_the_grid() {
        let mut t = Ticks::new(Timestamp(1000), Hz(1e7));
        assert!(t.due(Timestamp(1000)));
        assert_eq!(t.advance(Timestamp(1003)), 0);
        assert_eq!(t.next(), Timestamp(1100));
        assert!(!t.due(Timestamp(1099)));
        assert_eq!(t.advance(Timestamp(1050)), 0);
        assert_eq!(t.next(), Timestamp(1100));
    }

    #[test]
    fn late_advance_skips_missed_ticks() {
        let mut t = Ticks::new(Timestamp(0), Hz(1e8));
        t.advance(Timestamp(0));
        // Ticks 1, 2 and 3 are at 10, 20 and 30. At 35, tick 3 is consumed and 1 and 2 are
        // skipped.
        assert_eq!(t.advance(Timestamp(35)), 2);
        assert_eq!(t.next(), Timestamp(40));
        assert_eq!(t.index(), 4);
    }

    #[test]
    fn fractional_rates_stay_exact() {
        // 60000/1001 Hz ticks every 16_683_333.33 ns; tick 60_000 is exactly 1001 s.
        let rate = Hz(60000.0 / 1001.0);
        let mut t = Ticks::new(Timestamp(0), rate);
        for _ in 0..60_000u64 {
            let at = t.next();
            assert_eq!(t.advance(at), 0);
        }
        assert_eq!(t.next(), Timestamp(1_001_000_000_000));
    }
}
