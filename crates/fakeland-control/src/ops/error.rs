//! Errors of control operations and their wire codes.

use std::path::PathBuf;

use fakeland::{ActionError, CaptureError, SessionError, WaitError};
use thiserror::Error;

use crate::wire::{ErrorBody, ErrorCode};

/// A control operation that failed.
#[derive(Debug, Error)]
pub enum OpError {
    /// A session call failed.
    #[error(transparent)]
    Session(#[from] SessionError),
    /// The parameters are invalid.
    #[error("{0}")]
    InvalidParams(String),
    /// An app, run, capture or subscription does not exist.
    #[error("{0}")]
    NotFound(String),
    /// A run has not finished.
    #[error("run {0} has not finished")]
    Pending(u64),
    /// A wait ran out of time.
    #[error("timed out")]
    Timeout,
    /// A file operation failed.
    #[error("{path}: {error}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error.
        error: std::io::Error,
    },
    /// The operation failed for another reason.
    #[error("{0}")]
    Failed(String),
}

impl OpError {
    /// Builds a file error.
    pub fn io(path: impl Into<PathBuf>, error: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            error,
        }
    }

    /// Returns the wire code of the error.
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Session(e) => session_code(e),
            Self::InvalidParams(_) => ErrorCode::InvalidParams,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::Pending(_) => ErrorCode::Pending,
            Self::Timeout => ErrorCode::Timeout,
            Self::Io { .. } => ErrorCode::Io,
            Self::Failed(_) => ErrorCode::Failed,
        }
    }

    /// Converts the error into a wire error body.
    pub fn to_body(&self) -> ErrorBody {
        ErrorBody::new(self.code(), self.to_string())
    }
}

fn a11y_code(e: &fakeland::accessibility::A11yError) -> ErrorCode {
    use fakeland::accessibility::A11yError as E;
    match e {
        E::InvalidId(_)
        | E::UnknownName(_)
        | E::Pattern(_)
        | E::NoAction { .. }
        | E::Unsupported { .. } => ErrorCode::InvalidParams,
        E::NoApp(_) | E::NoFrame(_) | E::Node { .. } | E::NoBus(_) => ErrorCode::NotFound,
        _ => ErrorCode::Failed,
    }
}

fn session_code(e: &SessionError) -> ErrorCode {
    match e {
        SessionError::Closed => ErrorCode::Closed,
        SessionError::Timeout => ErrorCode::Timeout,
        SessionError::NoWindow => ErrorCode::NoWindow,
        SessionError::Match(_) | SessionError::Combo(_) => ErrorCode::InvalidParams,
        SessionError::Wait(w) => wait_code(w),
        SessionError::Capture(c) => capture_code(c),
        SessionError::Action(a) => match a {
            ActionError::NoWindow => ErrorCode::NoWindow,
            ActionError::Match(_) | ActionError::Expand(_) | ActionError::Unsupported(_) => {
                ErrorCode::InvalidParams
            }
            ActionError::Wait(w) => wait_code(w),
            ActionError::Capture(c) => capture_code(c),
            _ => ErrorCode::Failed,
        },
        SessionError::NoFrameCapture | SessionError::Trace(_) => ErrorCode::NotFound,
        SessionError::A11y(a) => a11y_code(a),
        SessionError::Record(r) => record_code(r),
        _ => ErrorCode::Failed,
    }
}

fn record_code(e: &fakeland::RecordError) -> ErrorCode {
    use fakeland::RecordError as E;
    match e {
        E::NoWindow => ErrorCode::NoWindow,
        E::NoOutput | E::NotFound => ErrorCode::NotFound,
        E::Match(_) | E::Format(_) => ErrorCode::InvalidParams,
        E::Io { .. } => ErrorCode::Io,
        E::Capture(c) => capture_code(c),
    }
}

fn wait_code(e: &WaitError) -> ErrorCode {
    match e {
        WaitError::Timeout => ErrorCode::Timeout,
        WaitError::NoWindow => ErrorCode::NoWindow,
        WaitError::Match(_) | WaitError::Condition(_) => ErrorCode::InvalidParams,
        WaitError::Capture(c) => capture_code(c),
    }
}

fn capture_code(e: &CaptureError) -> ErrorCode {
    match e {
        CaptureError::NoWindow => ErrorCode::NoWindow,
        CaptureError::NoOutput => ErrorCode::NotFound,
        CaptureError::Match(_) => ErrorCode::InvalidParams,
        _ => ErrorCode::Failed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_codes() {
        assert_eq!(
            OpError::from(SessionError::Timeout).code(),
            ErrorCode::Timeout
        );
        assert_eq!(
            OpError::from(SessionError::Closed).code(),
            ErrorCode::Closed
        );
        assert_eq!(
            OpError::from(SessionError::Wait(WaitError::NoWindow)).code(),
            ErrorCode::NoWindow
        );
        assert_eq!(OpError::Pending(3).to_body().code, ErrorCode::Pending);
        assert_eq!(
            OpError::Pending(3).to_body().message,
            "run 3 has not finished"
        );
    }
}
