//! The part of the accessibility tree that a call reads.

use fakeland_types::{ProcessId, WindowId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::NodeRef;

/// The root of a tree walk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Scope {
    /// Every accessible app, below the registry root.
    Desktop,
    /// The root of the app with this pid.
    Pid {
        /// The pid of the app.
        pid: ProcessId,
    },
    /// The root of the first app whose name contains this text, case-insensitively.
    AppName {
        /// A part of the app name.
        name: String,
    },
    /// The frame node of a compositor window.
    Window {
        /// The window.
        window: WindowId,
    },
    /// A node and its subtree.
    Node {
        /// The node.
        node: NodeRef,
    },
}
