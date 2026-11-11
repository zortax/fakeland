//! The error part of a response.

use std::fmt;

use serde::{Deserialize, Serialize};

/// The class of a failed request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The line is not valid JSON or not a request.
    InvalidRequest,
    /// The server has no method with this name.
    UnknownMethod,
    /// The parameters do not fit the method.
    InvalidParams,
    /// A wait ran out of time.
    Timeout,
    /// No window matches.
    NoWindow,
    /// An app, run, capture or subscription with this id does not exist.
    NotFound,
    /// A run has not finished yet.
    Pending,
    /// The session has shut down.
    Closed,
    /// A file could not be read or written.
    Io,
    /// The method failed for another reason.
    Failed,
}

impl ErrorCode {
    /// Returns the wire name of the code.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::UnknownMethod => "unknown_method",
            Self::InvalidParams => "invalid_params",
            Self::Timeout => "timeout",
            Self::NoWindow => "no_window",
            Self::NotFound => "not_found",
            Self::Pending => "pending",
            Self::Closed => "closed",
            Self::Io => "io",
            Self::Failed => "failed",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A failed request: a code for programs and a message for people.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorBody {
    /// The class of the error.
    pub code: ErrorCode,
    /// The error text.
    pub message: String,
}

impl ErrorBody {
    /// Builds an error body.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for ErrorBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ErrorBody {}
