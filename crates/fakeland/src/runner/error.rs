//! Errors that end a scenario run.

use fakeland_scenario::ScenarioError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::actions::ActionError;

/// The kind of a run failure, for reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RunErrorKind {
    /// The scenario is invalid.
    Invalid,
    /// A step failed.
    Step,
    /// The scenario deadline passed.
    Timeout,
    /// The caller cancelled the run.
    Cancelled,
    /// The session shut down.
    Closed,
}

/// A run that ended early.
#[derive(Debug, Error)]
pub enum RunError {
    /// The scenario is invalid.
    #[error(transparent)]
    Invalid(#[from] ScenarioError),
    /// A step failed.
    #[error("{path}: {source}")]
    Step {
        /// The step path.
        path: String,
        /// The failure.
        source: ActionError,
    },
    /// An anchor wait of a step failed.
    #[error("{path}: anchor: {source}")]
    Anchor {
        /// The step path.
        path: String,
        /// The failure.
        source: crate::waits::WaitError,
    },
    /// The scenario deadline passed.
    #[error("the scenario timed out")]
    Timeout,
    /// The caller cancelled the run.
    #[error("the run was cancelled")]
    Cancelled,
    /// The session shut down.
    #[error("the session shut down")]
    Closed,
}

impl RunError {
    /// Returns the kind of the failure.
    pub fn kind(&self) -> RunErrorKind {
        match self {
            Self::Invalid(_) => RunErrorKind::Invalid,
            Self::Step { .. } | Self::Anchor { .. } => RunErrorKind::Step,
            Self::Timeout => RunErrorKind::Timeout,
            Self::Cancelled => RunErrorKind::Cancelled,
            Self::Closed => RunErrorKind::Closed,
        }
    }
}
