//! The complete timing configuration of one output.

use fakeland_types::dur::{Dur, as_dur};
use fakeland_types::{Hz, Nanos};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{CallbackPolicy, ConfigError, Perturbation, TimingMode};

/// The timing configuration of one output. The default is a fixed 60 Hz grid that latches 1 ms
/// before each vblank and sends frame callbacks at the vblank.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TimingConfig {
    /// How the output refreshes.
    pub mode: TimingMode,
    /// The latch deadline is the vblank minus this offset. It applies to the fixed grid. VRR,
    /// manual and unthrottled outputs latch at the frame itself.
    #[serde(with = "as_dur")]
    #[schemars(with = "Dur")]
    pub latch_offset: Nanos,
    /// When frame callbacks reach clients.
    pub callbacks: CallbackPolicy,
    /// Seeded latch jitter and dropped vblanks.
    pub perturbation: Option<Perturbation>,
    /// Presentation feedback carries the zero-copy flag.
    pub zero_copy: bool,
}

impl Default for TimingConfig {
    fn default() -> Self {
        Self {
            mode: TimingMode::default(),
            latch_offset: Nanos::from_millis(1),
            callbacks: CallbackPolicy::AtVblank,
            perturbation: None,
            zero_copy: false,
        }
    }
}

impl TimingConfig {
    /// Builds the default configuration with a fixed rate.
    pub fn fixed(refresh: Hz) -> Self {
        Self::default().with_mode(TimingMode::fixed(refresh))
    }

    /// Builds the default configuration with a VRR range.
    pub fn vrr(min: Hz, max: Hz) -> Self {
        Self::default().with_mode(TimingMode::vrr(min, max))
    }

    /// Builds the default configuration without throttling.
    pub fn unthrottled() -> Self {
        Self::default().with_mode(TimingMode::Unthrottled)
    }

    /// Builds the default configuration with vblanks on demand.
    pub fn manual(nominal: Hz) -> Self {
        Self::default().with_mode(TimingMode::manual(nominal))
    }

    /// Replaces the mode.
    pub fn with_mode(mut self, mode: TimingMode) -> Self {
        self.mode = mode;
        self
    }

    /// Replaces the latch offset.
    pub fn with_latch_offset(mut self, offset: Nanos) -> Self {
        self.latch_offset = offset;
        self
    }

    /// Replaces the callback policy.
    pub fn with_callbacks(mut self, policy: CallbackPolicy) -> Self {
        self.callbacks = policy;
        self
    }

    /// Replaces the perturbation.
    pub fn with_perturbation(mut self, perturbation: Perturbation) -> Self {
        self.perturbation = Some(perturbation);
        self
    }

    /// Sets the zero-copy flag.
    pub fn with_zero_copy(mut self, zero_copy: bool) -> Self {
        self.zero_copy = zero_copy;
        self
    }

    /// Returns true when the vblank with this sequence number drops.
    pub fn drops(&self, seq: u64) -> bool {
        self.perturbation.is_some_and(|p| p.drops(seq))
    }

    /// Returns the latch jitter of the vblank with this sequence number.
    pub fn jitter(&self, seq: u64) -> Nanos {
        self.perturbation.map_or(Nanos::ZERO, |p| p.jitter(seq))
    }

    /// Checks that the scheduler can run this configuration.
    pub fn validate(&self) -> Result<(), ConfigError> {
        self.mode.validate()?;
        if let Some(period) = self.mode.nominal_period() {
            let offset = self.latch_offset;
            if offset.is_negative() || (self.is_fixed() && offset >= period) {
                return Err(ConfigError::LatchOffset {
                    offset_ns: offset.as_nanos(),
                    period_ns: period.as_nanos(),
                });
            }
        }
        if let CallbackPolicy::BeforeVblank { lead } = self.callbacks
            && lead.is_negative()
        {
            return Err(ConfigError::NegativeLead(lead.as_nanos()));
        }
        if let Some(p) = &self.perturbation {
            p.validate()?;
        }
        Ok(())
    }

    fn is_fixed(&self) -> bool {
        matches!(self.mode, TimingMode::Fixed { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_fixed_60() {
        let c = TimingConfig::default();
        assert_eq!(c.mode, TimingMode::Fixed { refresh: Hz(60.0) });
        assert_eq!(c.latch_offset, Nanos::from_millis(1));
        assert!(c.validate().is_ok());
    }

    #[test]
    fn latch_offset_must_fit_the_period() {
        let c = TimingConfig::fixed(Hz(500.0)).with_latch_offset(Nanos::from_millis(2));
        assert!(c.validate().is_err());
        let c = TimingConfig::default().with_latch_offset(Nanos(-1));
        assert!(c.validate().is_err());
    }

    #[test]
    fn json_reads_human_durations_and_defaults() {
        let c: TimingConfig = serde_json::from_str(
            r#"{"mode":{"type":"fixed","refresh":144.0},"latch_offset":"1.5ms",
                "callbacks":{"type":"before_vblank","lead":"4ms"},
                "perturbation":{"latch_jitter":"200us","drop_every":10,"seed":7}}"#,
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(c.mode, TimingMode::fixed(Hz(144.0)));
        assert_eq!(c.latch_offset, Nanos::from_micros(1500));
        assert_eq!(
            c.callbacks,
            CallbackPolicy::BeforeVblank {
                lead: Nanos::from_millis(4)
            }
        );
        let p = c.perturbation.unwrap_or_default();
        assert_eq!(p.latch_jitter, Nanos::from_micros(200));
        assert_eq!(p.drop_every, Some(10));
        assert!(!c.zero_copy);
        let empty: TimingConfig = serde_json::from_str("{}").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(empty, TimingConfig::default());
    }

    #[test]
    fn serde_round_trip() {
        let c = TimingConfig::vrr(Hz(48.0), Hz(165.0))
            .with_callbacks(CallbackPolicy::OnPresent)
            .with_perturbation(Perturbation {
                drop_probability: 0.1,
                seed: 1,
                ..Default::default()
            })
            .with_zero_copy(true);
        let json = serde_json::to_string(&c).unwrap_or_default();
        let back: TimingConfig = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(back, c);
        for policy in [
            CallbackPolicy::AtVblank,
            CallbackPolicy::OnPresent,
            CallbackPolicy::BeforeVblank {
                lead: Nanos::from_micros(3500),
            },
        ] {
            let json = serde_json::to_string(&policy).unwrap_or_default();
            let back: CallbackPolicy =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(back, policy);
        }
    }

    #[test]
    fn schema_names_the_fields() {
        let schema = schemars::schema_for!(TimingConfig);
        let text = serde_json::to_string(&schema).unwrap_or_default();
        assert!(text.contains("latch_offset") && text.contains("before_vblank"));
    }
}
