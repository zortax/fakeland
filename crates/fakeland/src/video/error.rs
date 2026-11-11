//! Errors of recording start and stop.

use std::path::PathBuf;

use thiserror::Error;

use crate::capture::CaptureError;
use crate::shell::MatchError;

/// A recording that cannot start or stop.
#[derive(Debug, Error)]
pub enum RecordError {
    /// No output matches.
    #[error("no such output")]
    NoOutput,
    /// No mapped window matches.
    #[error("no such window")]
    NoWindow,
    /// The window matcher is invalid.
    #[error(transparent)]
    Match(#[from] MatchError),
    /// The format or frame rate is unusable.
    #[error("{0}")]
    Format(String),
    /// The first capture failed.
    #[error(transparent)]
    Capture(#[from] CaptureError),
    /// The output file or the encoder does not open.
    #[error("cannot write {path}: {message}")]
    Io {
        /// The path.
        path: PathBuf,
        /// The error.
        message: String,
    },
    /// The recording id is unknown.
    #[error("no such recording")]
    NotFound,
}
