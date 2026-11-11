//! Limits and switches of a tree walk.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Limits of a tree walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TreeOptions {
    /// The deepest level below the root that the walk fetches. The root is level 0.
    pub max_depth: usize,
    /// The most nodes that the walk fetches.
    pub max_nodes: usize,
    /// The walk skips nodes without the `showing` state and their subtrees. The root is always
    /// fetched.
    pub showing_only: bool,
    /// The most characters of text contents per node. Zero skips text contents.
    pub text_limit: usize,
    /// The most D-Bus requests in flight.
    pub concurrency: usize,
}

impl Default for TreeOptions {
    fn default() -> Self {
        Self {
            max_depth: 32,
            max_nodes: 2000,
            showing_only: false,
            text_limit: 200,
            concurrency: 32,
        }
    }
}

impl TreeOptions {
    /// Sets the depth limit.
    pub fn depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Sets the node limit.
    pub fn nodes(mut self, max_nodes: usize) -> Self {
        self.max_nodes = max_nodes;
        self
    }

    /// Selects visible nodes only.
    pub fn showing(mut self, showing_only: bool) -> Self {
        self.showing_only = showing_only;
        self
    }
}
