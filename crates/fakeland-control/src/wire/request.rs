//! A request from a client.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One method call. The server answers with a response that carries the same id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    /// The client-chosen id of the call.
    pub id: u64,
    /// The method name.
    pub method: String,
    /// The method parameters. A missing value reads as `null`.
    #[serde(default)]
    pub params: Value,
}

impl Request {
    /// Builds a request.
    pub fn new(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            id,
            method: method.into(),
            params,
        }
    }
}
