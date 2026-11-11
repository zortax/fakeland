//! Errors of scenarios that do not parse or cannot run.

use crate::ExpandError;

/// A scenario that does not parse or cannot run.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ScenarioError {
    /// The JSON does not describe a scenario.
    #[error("invalid scenario JSON: {0}")]
    Json(String),
    /// An action does not expand.
    #[error("{location}: {source}")]
    Expand {
        /// The path to the action, for example `steps[2].steps[0]`.
        location: String,
        /// The cause.
        source: ExpandError,
    },
    /// A value is outside its range.
    #[error("{location}: {message}")]
    Invalid {
        /// The path to the value.
        location: String,
        /// What is wrong.
        message: String,
    },
    /// A key combination does not parse.
    #[error("{location}: {source}")]
    Combo {
        /// The path to the step.
        location: String,
        /// The cause.
        source: crate::ComboError,
    },
}
