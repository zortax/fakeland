//! The frame rate of a recording as an exact rational, and the slot times it defines. A rate
//! that comes from an output period puts every slot exactly on the vblank grid.

use fakeland_types::{Hz, Nanos};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const NS_PER_SEC: u128 = 1_000_000_000;

/// A frame rate of `num / den` frames per second.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct FrameRate {
    /// The numerator.
    pub num: u32,
    /// The denominator.
    pub den: u32,
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a.max(1)
}

impl FrameRate {
    /// Builds a reduced rate. Both parts must be positive.
    pub fn new(num: u64, den: u64) -> Option<Self> {
        if num == 0 || den == 0 {
            return None;
        }
        let g = gcd(num, den);
        Some(Self {
            num: u32::try_from(num / g).ok()?,
            den: u32::try_from(den / g).ok()?,
        })
    }

    /// Returns the rate of one frame per refresh period.
    pub fn from_period(period: Nanos) -> Option<Self> {
        let p = u64::try_from(period.as_nanos()).ok()?;
        Self::new(NS_PER_SEC as u64, p)
    }

    /// Returns a rate in Hz as a small rational: whole rates, NTSC rates `n * 1000 / 1001`, or
    /// millihertz.
    pub fn from_hz(hz: Hz) -> Option<Self> {
        let f = hz.0;
        if !(f.is_finite() && f > 0.0) {
            return None;
        }
        let whole = f.round();
        if (f - whole).abs() < 1e-6 {
            return Self::new(whole as u64, 1);
        }
        let ntsc = (f * 1.001).round();
        if (f - ntsc * 1000.0 / 1001.0).abs() < 1e-4 {
            return Self::new(ntsc as u64 * 1000, 1001);
        }
        Self::new((f * 1000.0).round() as u64, 1000)
    }

    /// Returns the rate in Hz.
    pub fn hz(self) -> Hz {
        Hz(f64::from(self.num) / f64::from(self.den))
    }

    /// Returns the slot period, rounded to nanoseconds.
    pub fn period(self) -> Nanos {
        let ns =
            (u128::from(self.den) * NS_PER_SEC + u128::from(self.num) / 2) / u128::from(self.num);
        Nanos(ns as i64)
    }

    /// Returns the offset of slot `i` from the first slot, rounded down to nanoseconds.
    pub fn offset(self, i: u64) -> Nanos {
        let ns = u128::from(i) * u128::from(self.den) * NS_PER_SEC / u128::from(self.num);
        Nanos(ns.min(i64::MAX as u128) as i64)
    }

    /// Returns the count of slots whose offset lies before `span`. The first slot has offset
    /// zero.
    pub fn slots_before(self, span: Nanos) -> u64 {
        if span.as_nanos() <= 0 {
            return 0;
        }
        let x = span.as_nanos() as u128 * u128::from(self.num);
        let d = u128::from(self.den) * NS_PER_SEC;
        x.div_ceil(d).min(u64::MAX as u128) as u64
    }

    /// Returns the rate as `num/den` for ffmpeg.
    pub fn ffmpeg(self) -> String {
        format!("{}/{}", self.num, self.den)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_from_hz_are_small_rationals() {
        assert_eq!(FrameRate::from_hz(Hz(60.0)), FrameRate::new(60, 1));
        assert_eq!(FrameRate::from_hz(Hz(59.94)), FrameRate::new(60000, 1001));
        assert_eq!(FrameRate::from_hz(Hz(23.976)), FrameRate::new(24000, 1001));
        assert_eq!(
            FrameRate::from_hz(Hz(143.912)),
            FrameRate::new(143912, 1000)
        );
        assert_eq!(FrameRate::from_hz(Hz(0.0)), None);
    }

    #[test]
    fn rates_from_periods_land_on_the_grid() {
        let r = FrameRate::from_period(Nanos(16_666_667)).unwrap();
        assert_eq!(r.offset(3), Nanos(50_000_001));
        assert_eq!(r.period(), Nanos(16_666_667));
        assert!((r.hz().0 - 60.0).abs() < 1e-4);
        assert_eq!(FrameRate::from_period(Nanos(0)), None);
    }

    #[test]
    fn counts_slots_strictly_before_a_span() {
        let r = FrameRate::new(60, 1).unwrap();
        assert_eq!(r.slots_before(Nanos(0)), 0);
        assert_eq!(r.slots_before(Nanos(1)), 1);
        // Slot 1 starts at 16_666_666 ns.
        assert_eq!(r.slots_before(Nanos(16_666_666)), 1);
        assert_eq!(r.slots_before(Nanos(16_666_667)), 2);
        assert_eq!(r.slots_before(Nanos::from_secs(1)), 60);
        assert_eq!(r.ffmpeg(), "60/1");
    }
}
