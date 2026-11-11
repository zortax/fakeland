//! Reads a trace file in JSONL or whole-document JSON form.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

use fakeland_trace::Trace;
use thiserror::Error;

/// A trace file that cannot be read.
#[derive(Debug, Error)]
pub enum ReadError {
    /// The file cannot be opened.
    #[error("{path}: {error}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error.
        error: std::io::Error,
    },
    /// The file is no trace in either form.
    #[error("{path}: not a trace: {message}")]
    Parse {
        /// The file.
        path: PathBuf,
        /// The parse error of the JSONL form.
        message: String,
    },
}

/// Reads a trace file. A file with one header line and record lines reads as JSONL; a file with
/// one JSON document reads as a whole trace.
pub fn read_trace(path: &Path) -> Result<Trace, ReadError> {
    let io = |error| ReadError::Io {
        path: path.to_path_buf(),
        error,
    };
    let mut text = String::new();
    BufReader::new(File::open(path).map_err(io)?)
        .read_to_string(&mut text)
        .map_err(io)?;
    match Trace::read_jsonl(text.as_bytes()) {
        Ok(trace) => Ok(trace),
        Err(jsonl) => Trace::read_json(text.as_bytes()).map_err(|_| ReadError::Parse {
            path: path.to_path_buf(),
            message: jsonl.to_string(),
        }),
    }
}
