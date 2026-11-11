//! Timestamps on the wire. A timestamp travels as seconds in a high and a low 32-bit half plus a
//! nanosecond remainder, the layout of wp_presentation.

use fakeland_types::Timestamp;

use super::id::{join_u64, split_u64};

const NANOS_PER_SEC: u32 = 1_000_000_000;

/// A `CLOCK_MONOTONIC` timestamp in its wire layout.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct WireTime {
    /// High 32 bits of the seconds.
    pub sec_hi: u32,
    /// Low 32 bits of the seconds.
    pub sec_lo: u32,
    /// Nanoseconds part, below one second for a valid value.
    pub nsec: u32,
}

/// A wire timestamp that has no `Timestamp` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TimeError {
    /// The nanoseconds part is one second or more.
    #[error("the nanosecond part {0} is one second or more")]
    NsecOutOfRange(u32),
    /// The time is too large for a 64-bit nanosecond count.
    #[error("the time does not fit in 64-bit nanoseconds")]
    Overflow,
}

impl WireTime {
    /// Encodes a timestamp.
    pub const fn from_timestamp(t: Timestamp) -> Self {
        let (secs, nsec) = t.to_secs_nanos();
        let (sec_hi, sec_lo) = split_u64(secs);
        Self {
            sec_hi,
            sec_lo,
            nsec,
        }
    }

    /// Decodes the timestamp.
    pub fn to_timestamp(self) -> Result<Timestamp, TimeError> {
        if self.nsec >= NANOS_PER_SEC {
            return Err(TimeError::NsecOutOfRange(self.nsec));
        }
        join_u64(self.sec_hi, self.sec_lo)
            .checked_mul(u64::from(NANOS_PER_SEC))
            .and_then(|ns| ns.checked_add(u64::from(self.nsec)))
            .map(Timestamp)
            .ok_or(TimeError::Overflow)
    }
}

impl From<Timestamp> for WireTime {
    fn from(t: Timestamp) -> Self {
        Self::from_timestamp(t)
    }
}

impl TryFrom<WireTime> for Timestamp {
    type Error = TimeError;

    fn try_from(w: WireTime) -> Result<Self, TimeError> {
        w.to_timestamp()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timestamps_round_trip() {
        for ns in [
            0,
            1,
            999_999_999,
            1_000_000_000,
            123_456_789_012_345,
            u64::MAX,
        ] {
            let t = Timestamp(ns);
            assert_eq!(WireTime::from_timestamp(t).to_timestamp(), Ok(t));
        }
    }

    #[test]
    fn splits_seconds_and_nanoseconds() {
        let w = WireTime::from_timestamp(Timestamp(5_000_000_007));
        assert_eq!(
            w,
            WireTime {
                sec_hi: 0,
                sec_lo: 5,
                nsec: 7
            }
        );
    }

    #[test]
    fn rejects_out_of_range_nanoseconds() {
        let w = WireTime {
            sec_hi: 0,
            sec_lo: 1,
            nsec: NANOS_PER_SEC,
        };
        assert_eq!(
            w.to_timestamp(),
            Err(TimeError::NsecOutOfRange(NANOS_PER_SEC))
        );
    }

    #[test]
    fn rejects_times_past_u64_nanoseconds() {
        let w = WireTime {
            sec_hi: u32::MAX,
            sec_lo: u32::MAX,
            nsec: 0,
        };
        assert_eq!(w.to_timestamp(), Err(TimeError::Overflow));
    }
}
