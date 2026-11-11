//! Errors of the Chrome trace export.

use std::io;

/// An error of the Chrome trace export.
#[derive(Debug, thiserror::Error)]
pub enum PerfettoError {
    /// The writer fails.
    #[error("chrome trace i/o failed")]
    Io(#[from] io::Error),
    /// The JSON serialization fails.
    #[error("chrome trace serialization failed")]
    Json(#[from] serde_json::Error),
}
