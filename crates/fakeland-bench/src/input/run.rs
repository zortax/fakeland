//! The serializable part of a scenario run result.

use fakeland::{RunResult, StepTiming, TriggerFiring};
use serde::{Deserialize, Serialize};

/// The outcome of a scenario run without its trace and images.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    /// The scenario name.
    pub name: String,
    /// The run finished every step.
    pub ok: bool,
    /// The error that ended the run.
    pub error: Option<String>,
    /// The executed steps in order.
    pub steps: Vec<StepTiming>,
    /// Every trigger firing.
    pub triggers: Vec<TriggerFiring>,
    /// The count of screenshots that capture steps took.
    pub captures: usize,
}

impl From<&RunResult> for RunSummary {
    fn from(r: &RunResult) -> Self {
        Self {
            name: r.name.clone(),
            ok: r.ok,
            error: r.error.clone(),
            steps: r.steps.clone(),
            triggers: r.triggers.clone(),
            captures: r.captures.len(),
        }
    }
}
