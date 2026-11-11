//! Errors of preset lookup and parameters.

use thiserror::Error;

/// An error while building a bench from a preset.
#[derive(Debug, Error)]
pub enum PresetError {
    /// No preset has the name.
    #[error("no preset named {0:?}")]
    Unknown(String),
    /// A parameter name is unknown.
    #[error("preset {preset}: unknown parameter {key:?}; known: {known}")]
    UnknownParam {
        /// The preset.
        preset: String,
        /// The parameter.
        key: String,
        /// The known parameters, comma separated.
        known: String,
    },
    /// A parameter value is invalid.
    #[error("preset {preset}: {message}")]
    Params {
        /// The preset.
        preset: String,
        /// What is wrong.
        message: String,
    },
}

impl PresetError {
    /// Builds a parameter error.
    pub fn params(preset: &str, message: impl Into<String>) -> Self {
        Self::Params {
            preset: preset.to_string(),
            message: message.into(),
        }
    }
}
