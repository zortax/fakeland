//! Errors of client calls.

use std::path::PathBuf;

use thiserror::Error;

use crate::wire::ErrorBody;

/// A client call that failed.
#[derive(Debug, Error)]
pub enum ClientError {
    /// No control socket is known.
    #[error(
        "no control socket found: pass --socket, set FAKELAND_SOCKET, or start `fakeland serve`"
    )]
    NoSocket,
    /// More than one control socket is running.
    #[error("several control sockets run; pick one with --socket: {}", list(.0))]
    Ambiguous(Vec<PathBuf>),
    /// The socket cannot be reached.
    #[error("{path}: {error}")]
    Connect {
        /// The socket path.
        path: PathBuf,
        /// The error.
        error: std::io::Error,
    },
    /// Writing the request failed.
    #[error("cannot send the request: {0}")]
    Io(#[from] std::io::Error),
    /// The connection closed before the response.
    #[error("the control connection closed")]
    Closed,
    /// The server answered with an error.
    #[error("{0}")]
    Remote(ErrorBody),
    /// A message does not fit its type.
    #[error("bad message: {0}")]
    Json(#[from] serde_json::Error),
}

fn list(paths: &[PathBuf]) -> String {
    paths
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}
