//! Points in time on `CLOCK_MONOTONIC` and signed spans between them, both in nanoseconds.

use std::fmt;
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use std::time::Duration;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A point in time on `CLOCK_MONOTONIC`, in nanoseconds since boot.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(transparent)]
pub struct Timestamp(pub u64);

/// A signed span of time in nanoseconds.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
)]
#[serde(transparent)]
pub struct Nanos(pub i64);

impl Timestamp {
    /// The zero timestamp.
    pub const ZERO: Self = Self(0);

    /// Returns the timestamp in nanoseconds.
    pub const fn as_nanos(self) -> u64 {
        self.0
    }

    /// Returns the span from `earlier` to `self`. The span is negative when `earlier` is later.
    pub fn since(self, earlier: Self) -> Nanos {
        Nanos(self.0 as i64 - earlier.0 as i64)
    }

    /// Returns the timestamp in milliseconds, truncated to the 32-bit Wayland event time.
    pub const fn wayland_millis(self) -> u32 {
        (self.0 / 1_000_000) as u32
    }

    /// Returns the seconds and the nanosecond remainder.
    pub const fn to_secs_nanos(self) -> (u64, u32) {
        (self.0 / 1_000_000_000, (self.0 % 1_000_000_000) as u32)
    }

    /// Builds a timestamp from seconds and a nanosecond remainder.
    pub const fn from_secs_nanos(secs: u64, nanos: u32) -> Self {
        Self(secs * 1_000_000_000 + nanos as u64)
    }

    /// Returns the earlier of two timestamps.
    pub fn min(self, other: Self) -> Self {
        Ord::min(self, other)
    }

    /// Returns the later of two timestamps.
    pub fn max(self, other: Self) -> Self {
        Ord::max(self, other)
    }
}

impl Nanos {
    /// The zero span.
    pub const ZERO: Self = Self(0);

    /// Builds a span from nanoseconds.
    pub const fn from_nanos(ns: i64) -> Self {
        Self(ns)
    }

    /// Builds a span from microseconds.
    pub const fn from_micros(us: i64) -> Self {
        Self(us * 1_000)
    }

    /// Builds a span from milliseconds.
    pub const fn from_millis(ms: i64) -> Self {
        Self(ms * 1_000_000)
    }

    /// Builds a span from seconds.
    pub const fn from_secs(s: i64) -> Self {
        Self(s * 1_000_000_000)
    }

    /// Builds a span from fractional milliseconds, rounded to the nearest nanosecond.
    pub fn from_millis_f64(ms: f64) -> Self {
        Self((ms * 1e6).round() as i64)
    }

    /// Builds a span from fractional seconds, rounded to the nearest nanosecond.
    pub fn from_secs_f64(s: f64) -> Self {
        Self((s * 1e9).round() as i64)
    }

    /// Returns the span in nanoseconds.
    pub const fn as_nanos(self) -> i64 {
        self.0
    }

    /// Returns the span in fractional microseconds.
    pub fn as_micros_f64(self) -> f64 {
        self.0 as f64 / 1e3
    }

    /// Returns the span in fractional milliseconds.
    pub fn as_millis_f64(self) -> f64 {
        self.0 as f64 / 1e6
    }

    /// Returns the span in fractional seconds.
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1e9
    }

    /// Returns true for a negative span.
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }

    /// Returns the span as a `Duration`, clamped at zero.
    pub fn to_duration(self) -> Duration {
        Duration::from_nanos(self.0.max(0) as u64)
    }

    /// Returns the absolute span.
    pub const fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Multiplies the span by a factor, rounded to the nearest nanosecond.
    pub fn mul_f64(self, factor: f64) -> Self {
        Self((self.0 as f64 * factor).round() as i64)
    }
}

impl From<Duration> for Nanos {
    fn from(d: Duration) -> Self {
        Self(d.as_nanos().min(i64::MAX as u128) as i64)
    }
}

impl Add<Nanos> for Timestamp {
    type Output = Timestamp;
    fn add(self, rhs: Nanos) -> Timestamp {
        Timestamp(self.0.saturating_add_signed(rhs.0))
    }
}

impl AddAssign<Nanos> for Timestamp {
    fn add_assign(&mut self, rhs: Nanos) {
        *self = *self + rhs;
    }
}

impl Sub<Nanos> for Timestamp {
    type Output = Timestamp;
    fn sub(self, rhs: Nanos) -> Timestamp {
        Timestamp(self.0.saturating_add_signed(-rhs.0))
    }
}

impl SubAssign<Nanos> for Timestamp {
    fn sub_assign(&mut self, rhs: Nanos) {
        *self = *self - rhs;
    }
}

impl Sub for Timestamp {
    type Output = Nanos;
    fn sub(self, rhs: Timestamp) -> Nanos {
        self.since(rhs)
    }
}

impl Add for Nanos {
    type Output = Nanos;
    fn add(self, rhs: Nanos) -> Nanos {
        Nanos(self.0 + rhs.0)
    }
}

impl AddAssign for Nanos {
    fn add_assign(&mut self, rhs: Nanos) {
        self.0 += rhs.0;
    }
}

impl Sub for Nanos {
    type Output = Nanos;
    fn sub(self, rhs: Nanos) -> Nanos {
        Nanos(self.0 - rhs.0)
    }
}

impl SubAssign for Nanos {
    fn sub_assign(&mut self, rhs: Nanos) {
        self.0 -= rhs.0;
    }
}

impl Mul<i64> for Nanos {
    type Output = Nanos;
    fn mul(self, rhs: i64) -> Nanos {
        Nanos(self.0 * rhs)
    }
}

impl Div<i64> for Nanos {
    type Output = Nanos;
    fn div(self, rhs: i64) -> Nanos {
        Nanos(self.0 / rhs)
    }
}

impl Neg for Nanos {
    type Output = Nanos;
    fn neg(self) -> Nanos {
        Nanos(-self.0)
    }
}

impl fmt::Display for Nanos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3} ms", self.as_millis_f64())
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (s, ns) = self.to_secs_nanos();
        write!(f, "{s}.{ns:09}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_between_timestamps_are_signed() {
        let a = Timestamp(1_000);
        let b = Timestamp(4_000);
        assert_eq!(b - a, Nanos(3_000));
        assert_eq!(a - b, Nanos(-3_000));
        assert_eq!(a + Nanos(3_000), b);
        assert_eq!(b - Nanos(3_000), a);
        assert_eq!(Nanos(3_000) * 4, Nanos(12_000));
        assert_eq!(Nanos(3_000) / 4, Nanos(750));
    }

    #[test]
    fn wayland_time_is_milliseconds() {
        assert_eq!(Timestamp(5_123_456_789).wayland_millis(), 5_123);
    }

    #[test]
    fn secs_nanos_round_trip() {
        let t = Timestamp(12_345_678_901);
        let (s, ns) = t.to_secs_nanos();
        assert_eq!(Timestamp::from_secs_nanos(s, ns), t);
    }
}
