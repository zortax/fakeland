//! Refresh rates in hertz and their periods.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Nanos;

/// A refresh rate in hertz. Fractional rates such as 59.94 Hz are exact to the millihertz.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Hz(pub f64);

impl Hz {
    /// Returns the period of one refresh cycle, rounded to the nearest nanosecond.
    pub fn period(self) -> Nanos {
        Nanos((1e9 / self.0).round() as i64)
    }

    /// Returns the rate of a period.
    pub fn from_period(period: Nanos) -> Self {
        Self(1e9 / period.as_nanos() as f64)
    }

    /// Returns the rate in millihertz, as `wl_output.mode` reports it.
    pub fn millihertz(self) -> i32 {
        (self.0 * 1000.0).round() as i32
    }

    /// Builds a rate from millihertz.
    pub fn from_millihertz(mhz: i32) -> Self {
        Self(mhz as f64 / 1000.0)
    }
}

impl fmt::Display for Hz {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.3} Hz", self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn period_of_common_rates() {
        assert_eq!(Hz(60.0).period(), Nanos(16_666_667));
        assert_eq!(Hz(144.0).period(), Nanos(6_944_444));
        assert_eq!(Hz(59.94).millihertz(), 59_940);
    }
}
