//! Seeded perturbation of a display clock: latch jitter and dropped vblanks.

use fakeland_types::dur::{Dur, as_dur};
use fakeland_types::{Nanos, SplitMix64};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::ConfigError;

const SALT_DROP: u64 = 0x6472_6f70;
const SALT_JITTER: u64 = 0x6a69_7474;

/// Seeded disturbances of the compositor's latch.
///
/// A dropped vblank latches nothing, so content repeats. Presentation times stay on the ideal
/// grid. Every decision is a pure function of the seed and the vblank sequence number.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Perturbation {
    /// The latch moves by a uniform offset in `-latch_jitter..=latch_jitter`.
    #[serde(with = "as_dur")]
    #[schemars(with = "Dur")]
    pub latch_jitter: Nanos,
    /// Every vblank whose sequence number is a multiple of this value drops.
    pub drop_every: Option<u32>,
    /// Each vblank drops with this probability.
    pub drop_probability: f64,
    /// The seed of every random decision.
    pub seed: u64,
}

impl Perturbation {
    /// Returns true when the vblank with this sequence number drops.
    pub fn drops(&self, seq: u64) -> bool {
        let periodic = self
            .drop_every
            .is_some_and(|k| k > 0 && seq.is_multiple_of(u64::from(k)));
        let random = self.drop_probability > 0.0
            && SplitMix64::hash_f64(self.seed, SALT_DROP, seq) < self.drop_probability;
        periodic || random
    }

    /// Returns the latch offset for the vblank with this sequence number.
    pub fn jitter(&self, seq: u64) -> Nanos {
        if self.latch_jitter.as_nanos() <= 0 {
            return Nanos::ZERO;
        }
        let unit = SplitMix64::hash_f64(self.seed, SALT_JITTER, seq) * 2.0 - 1.0;
        self.latch_jitter.mul_f64(unit)
    }

    pub(crate) fn validate(&self) -> Result<(), ConfigError> {
        if self.latch_jitter.is_negative() {
            return Err(ConfigError::NegativeJitter(self.latch_jitter.as_nanos()));
        }
        if !(0.0..1.0).contains(&self.drop_probability) {
            return Err(ConfigError::DropProbability(self.drop_probability));
        }
        if let Some(k) = self.drop_every
            && k < 2
        {
            return Err(ConfigError::DropEvery(k));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn periodic_drops() {
        let p = Perturbation {
            drop_every: Some(4),
            ..Default::default()
        };
        let dropped: Vec<u64> = (1..=12).filter(|s| p.drops(*s)).collect();
        assert_eq!(dropped, vec![4, 8, 12]);
    }

    #[test]
    fn random_drops_follow_the_probability_and_the_seed() {
        let p = Perturbation {
            drop_probability: 0.25,
            seed: 9,
            ..Default::default()
        };
        let count = (0..10_000).filter(|s| p.drops(*s)).count();
        assert!((2_200..2_800).contains(&count), "{count}");
        let q = Perturbation { seed: 9, ..p };
        assert!((0..100).all(|s| p.drops(s) == q.drops(s)));
    }

    #[test]
    fn jitter_stays_in_range() {
        let p = Perturbation {
            latch_jitter: Nanos::from_micros(500),
            seed: 3,
            ..Default::default()
        };
        let mut min = i64::MAX;
        let mut max = i64::MIN;
        for s in 0..5_000 {
            let j = p.jitter(s).as_nanos();
            assert!(j.abs() <= 500_000);
            min = min.min(j);
            max = max.max(j);
        }
        assert!(min < -400_000 && max > 400_000);
        assert_eq!(Perturbation::default().jitter(5), Nanos::ZERO);
    }

    #[test]
    fn validation() {
        let bad = |p: Perturbation| p.validate().is_err();
        assert!(bad(Perturbation {
            drop_probability: 1.0,
            ..Default::default()
        }));
        assert!(bad(Perturbation {
            drop_every: Some(1),
            ..Default::default()
        }));
        assert!(bad(Perturbation {
            latch_jitter: Nanos(-1),
            ..Default::default()
        }));
        assert!(Perturbation::default().validate().is_ok());
    }
}
