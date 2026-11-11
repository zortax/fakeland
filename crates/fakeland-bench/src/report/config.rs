//! The bench configuration that a report records.

use fakeland::OutputState;
use fakeland_scenario::{PhaseSpec, WindowRef};
use fakeland_types::Dur;
use serde::{Deserialize, Serialize};

/// The settings of a bench run.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BenchConfig {
    /// The measured iterations per variant.
    pub iterations: u32,
    /// The warmup iterations per variant.
    pub warmup: u32,
    /// The pause before each iteration.
    pub settle: Dur,
    /// The quiet time the target needs before each iteration.
    pub quiet: Option<Dur>,
    /// The phase plan of the first input.
    pub phase: Option<PhaseSpec>,
    /// The seed.
    pub seed: u64,
    /// The variant labels in order.
    pub variants: Vec<String>,
    /// The target window matcher.
    pub target: Option<WindowRef>,
    /// The section names in order.
    pub analyzers: Vec<String>,
    /// The outputs when the bench started.
    pub outputs: Vec<OutputState>,
    /// The wake lateness that makes a run invalid.
    pub lateness_threshold: Dur,
}
