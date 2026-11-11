//! The refresh mode of an output: a fixed rate, a variable rate range, no throttling, or vblanks
//! on demand.

use fakeland_types::{Hz, Nanos};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::ConfigError;

/// How an output refreshes.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TimingMode {
    /// Vblanks on a fixed grid at a constant rate.
    Fixed {
        /// The refresh rate.
        refresh: Hz,
    },
    /// Variable refresh. A ready update presents as soon as the shortest period allows. The frame
    /// repeats when no update arrives within the longest period.
    Vrr {
        /// The lowest rate. Its period is the longest gap between frames.
        min: Hz,
        /// The highest rate. Its period is the shortest gap between frames.
        max: Hz,
    },
    /// Each update presents as soon as it is ready, without vsync.
    Unthrottled,
    /// Vblanks happen only when the caller steps the output.
    Manual {
        /// The rate that the output reports to clients.
        nominal: Hz,
    },
}

impl Default for TimingMode {
    fn default() -> Self {
        Self::Fixed { refresh: Hz(60.0) }
    }
}

impl TimingMode {
    /// Builds a fixed mode.
    pub fn fixed(refresh: Hz) -> Self {
        Self::Fixed { refresh }
    }

    /// Builds a VRR mode.
    pub fn vrr(min: Hz, max: Hz) -> Self {
        Self::Vrr { min, max }
    }

    /// Builds a manual mode.
    pub fn manual(nominal: Hz) -> Self {
        Self::Manual { nominal }
    }

    /// Returns the rate that the output reports to clients: the fixed rate, the highest VRR rate
    /// or the nominal manual rate. Unthrottled mode has no rate.
    pub fn nominal_rate(&self) -> Option<Hz> {
        match *self {
            Self::Fixed { refresh } => Some(refresh),
            Self::Vrr { max, .. } => Some(max),
            Self::Unthrottled => None,
            Self::Manual { nominal } => Some(nominal),
        }
    }

    /// Returns the period of the nominal rate.
    pub fn nominal_period(&self) -> Option<Nanos> {
        self.nominal_rate().map(Hz::period)
    }

    pub(crate) fn validate(&self) -> Result<(), ConfigError> {
        let check = |hz: Hz| {
            if hz.0.is_finite() && hz.0 > 0.0 && hz.period().as_nanos() > 0 {
                Ok(())
            } else {
                Err(ConfigError::InvalidRate(hz.0))
            }
        };
        match *self {
            Self::Fixed { refresh } => check(refresh),
            Self::Vrr { min, max } => {
                check(min)?;
                check(max)?;
                if min.0 > max.0 {
                    return Err(ConfigError::InvertedVrrRange {
                        min: min.0,
                        max: max.0,
                    });
                }
                Ok(())
            }
            Self::Unthrottled => Ok(()),
            Self::Manual { nominal } => check(nominal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nominal_rates() {
        assert_eq!(TimingMode::default().nominal_rate(), Some(Hz(60.0)));
        assert_eq!(
            TimingMode::vrr(Hz(48.0), Hz(144.0)).nominal_rate(),
            Some(Hz(144.0))
        );
        assert_eq!(TimingMode::Unthrottled.nominal_period(), None);
    }

    #[test]
    fn validation() {
        assert!(TimingMode::fixed(Hz(0.0)).validate().is_err());
        assert!(TimingMode::fixed(Hz(f64::NAN)).validate().is_err());
        assert!(TimingMode::vrr(Hz(144.0), Hz(48.0)).validate().is_err());
        assert!(TimingMode::vrr(Hz(48.0), Hz(144.0)).validate().is_ok());
        assert!(TimingMode::manual(Hz(-1.0)).validate().is_err());
    }

    #[test]
    fn serde_uses_a_type_tag() {
        let json = serde_json::to_string(&TimingMode::vrr(Hz(48.0), Hz(144.0))).unwrap_or_default();
        assert_eq!(json, r#"{"type":"vrr","min":48.0,"max":144.0}"#);
        let back: TimingMode =
            serde_json::from_str(r#"{"type":"unthrottled"}"#).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back, TimingMode::Unthrottled);
    }
}
