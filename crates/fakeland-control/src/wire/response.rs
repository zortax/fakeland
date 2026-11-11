//! A response to one request.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ErrorBody;

/// The answer to a request: its id, then either `result` or `error`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Response {
    /// The id of the request.
    pub id: u64,
    /// The outcome.
    #[serde(flatten)]
    pub body: ResponseBody,
}

/// The outcome of a request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseBody {
    /// The method result.
    Result(Value),
    /// The method failed.
    Error(ErrorBody),
}

impl Response {
    /// Builds a successful response.
    pub fn ok(id: u64, result: Value) -> Self {
        Self {
            id,
            body: ResponseBody::Result(result),
        }
    }

    /// Builds a failed response.
    pub fn error(id: u64, error: ErrorBody) -> Self {
        Self {
            id,
            body: ResponseBody::Error(error),
        }
    }

    /// Returns the result or the error.
    pub fn into_result(self) -> Result<Value, ErrorBody> {
        match self.body {
            ResponseBody::Result(v) => Ok(v),
            ResponseBody::Error(e) => Err(e),
        }
    }
}
