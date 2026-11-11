//! Latency probes: inject an input at a chosen vblank phase and detect the visible response.

use fakeland_types::{Dur, Nanos, SplitMix64};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Action, PixelCondition, Roi};

fn default_threshold() -> f64 {
    0.001
}

fn default_timeout() -> Dur {
    Dur::from_secs(1)
}

fn default_settle() -> Dur {
    Dur::from_millis(100)
}

fn one() -> u32 {
    1
}

const SALT_PHASE: u64 = 0x7068_6173;

/// What counts as the response to a probe input.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Detector {
    /// The first commit with damage in the region.
    AnyDamage {
        /// The region.
        roi: Roi,
    },
    /// The first presented frame whose region pixels changed.
    PixelsChanged {
        /// The region.
        roi: Roi,
        /// The fraction of region pixels that must change, in `0..=1`.
        #[serde(default = "default_threshold")]
        threshold: f64,
    },
    /// The first presented frame whose region meets a condition.
    PixelsMatch {
        /// The region.
        roi: Roi,
        /// The condition.
        condition: PixelCondition,
    },
    /// The app reports the input as handled through `fakeland_marks_v1`.
    AppMark {
        /// A mark name to wait for. `None` waits for `input_handled` of the injected event.
        #[serde(default)]
        name: Option<String>,
    },
}

/// Where in the refresh cycle each probe input goes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PhaseSpec {
    /// As soon as the settle time ends.
    #[default]
    Immediate,
    /// A fixed time after a vblank.
    Fixed {
        /// The offset after the vblank.
        offset: Dur,
    },
    /// A seeded uniform phase in the period.
    Random {
        /// The seed.
        seed: u64,
    },
    /// Evenly spaced phases: iteration `i` uses `(i mod steps) / steps` of the period.
    Sweep {
        /// The number of phases.
        steps: u32,
    },
}

impl PhaseSpec {
    /// Returns the phase after a vblank for an iteration, or `None` for immediate injection.
    pub fn phase_for(&self, iteration: u32, period: Nanos) -> Option<Nanos> {
        let p = period.as_nanos().max(1);
        match *self {
            Self::Immediate => None,
            Self::Fixed { offset } => Some(Nanos(offset.0.as_nanos().rem_euclid(p))),
            Self::Random { seed } => {
                let u = SplitMix64::hash_f64(seed, SALT_PHASE, u64::from(iteration));
                Some(Nanos(((u * p as f64) as i64).min(p - 1)))
            }
            Self::Sweep { steps } => {
                let steps = i64::from(steps.max(1));
                let i = i64::from(iteration) % steps;
                Some(Nanos(p * i / steps))
            }
        }
    }
}

/// A latency probe. Each repeat waits for `settle` without commits, injects `input` at the phase,
/// and waits up to `timeout` for the detector. `alternate` runs between repeats to restore the
/// state, so each repeat sees the same change.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ProbeSpec {
    /// The input action.
    pub input: Box<Action>,
    /// The response detector.
    pub detector: Detector,
    /// The deadline of each repeat.
    #[serde(default = "default_timeout")]
    pub timeout: Dur,
    /// The quiet time before each injection.
    #[serde(default = "default_settle")]
    pub settle: Dur,
    /// The injection phase.
    #[serde(default)]
    pub phase: PhaseSpec,
    /// The number of repeats.
    #[serde(default = "one")]
    pub repeat: u32,
    /// The action that restores the state between repeats.
    #[serde(default)]
    pub alternate: Option<Box<Action>>,
    /// A name for reports.
    #[serde(default)]
    pub label: Option<String>,
}

impl ProbeSpec {
    /// Builds a probe with one repeat and default timing.
    pub fn new(input: Action, detector: Detector) -> Self {
        Self {
            input: Box::new(input),
            detector,
            timeout: default_timeout(),
            settle: default_settle(),
            phase: PhaseSpec::Immediate,
            repeat: 1,
            alternate: None,
            label: None,
        }
    }

    /// Sets the repeat count.
    pub fn repeat(mut self, n: u32) -> Self {
        self.repeat = n;
        self
    }

    /// Sets the phase.
    pub fn phase(mut self, phase: PhaseSpec) -> Self {
        self.phase = phase;
        self
    }

    /// Sets the quiet time before each injection. Zero injects without waiting, which an app that
    /// animates needs, since it never goes quiet.
    pub fn settle(mut self, settle: Dur) -> Self {
        self.settle = settle;
        self
    }

    /// Sets the restoring action.
    pub fn alternate(mut self, action: Action) -> Self {
        self.alternate = Some(Box::new(action));
        self
    }

    /// Sets the label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phases() {
        let p = Nanos(16_000_000);
        assert_eq!(PhaseSpec::Immediate.phase_for(3, p), None);
        assert_eq!(
            PhaseSpec::Fixed {
                offset: Dur::from_millis(20)
            }
            .phase_for(0, p),
            Some(Nanos(4_000_000))
        );
        let sweep: Vec<_> = (0..5)
            .map(|i| PhaseSpec::Sweep { steps: 4 }.phase_for(i, p))
            .collect();
        assert_eq!(
            sweep,
            [0, 4_000_000, 8_000_000, 12_000_000, 0].map(|n| Some(Nanos(n)))
        );
        let r = PhaseSpec::Random { seed: 5 };
        let a: Vec<_> = (0..50).filter_map(|i| r.phase_for(i, p)).collect();
        assert!(a.iter().all(|n| (0..16_000_000).contains(&n.as_nanos())));
        assert_eq!(
            a,
            (0..50)
                .filter_map(|i| r.phase_for(i, p))
                .collect::<Vec<_>>()
        );
        assert!(a.windows(2).any(|w| w[0] != w[1]));
    }
}
