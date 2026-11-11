//! Errors of trace reading and writing.

use std::io;

/// An error of trace reading or writing.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    /// The reader or writer fails.
    #[error("trace i/o failed")]
    Io(#[from] io::Error),
    /// A JSON value fails to parse or serialize.
    #[error("trace JSON is invalid at line {line}")]
    Json {
        /// The 1-based line number, or zero for a whole JSON document.
        line: usize,
        /// The parser error.
        #[source]
        source: serde_json::Error,
    },
    /// The input holds no header line.
    #[error("trace has no header")]
    MissingHeader,
    /// The trace has a different format version.
    #[error("trace format version {found} is not supported, this build reads version {expected}")]
    Version {
        /// The version of the trace.
        found: u32,
        /// The version this build reads.
        expected: u32,
    },
}
