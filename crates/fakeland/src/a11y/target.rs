//! What an accessibility call reads: a compositor window, an app or a node.

use fakeland_a11y::NodeRef;
use fakeland_scenario::WindowRef;
use fakeland_types::ProcessId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The part of the accessibility tree that a call reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum A11yTarget {
    /// The frame of a compositor window.
    Window {
        /// The window. The default is the focused window.
        #[serde(default)]
        window: WindowRef,
    },
    /// The app with this pid.
    Pid {
        /// The pid of the app.
        pid: ProcessId,
    },
    /// The first app whose name contains this text, case-insensitively.
    App {
        /// A part of the app name.
        name: String,
    },
    /// A node and its subtree.
    Node {
        /// The node.
        node: NodeRef,
    },
    /// Every accessible app.
    Desktop,
}

impl Default for A11yTarget {
    fn default() -> Self {
        Self::Window {
            window: WindowRef::Focused,
        }
    }
}

impl From<WindowRef> for A11yTarget {
    fn from(window: WindowRef) -> Self {
        Self::Window { window }
    }
}

impl From<NodeRef> for A11yTarget {
    fn from(node: NodeRef) -> Self {
        Self::Node { node }
    }
}

/// How a click reaches a node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ClickVia {
    /// The activation action of the node, else a pointer click at its center.
    #[default]
    Auto,
    /// The activation action of the node.
    Action,
    /// A pointer click at the center of the node bounds.
    Pointer,
}

/// How a text change reached a node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TextMethod {
    /// The EditableText interface replaced the contents.
    EditableText,
    /// The node took focus, `ctrl+a` selected the contents, and the keyboard typed the text.
    Typed,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_round_trip() {
        let t: A11yTarget = serde_json::from_str(r#"{"type":"window"}"#).unwrap();
        assert_eq!(t, A11yTarget::default());
        let n: A11yTarget = serde_json::from_str(r#"{"type":"node","node":"1.2:3"}"#).unwrap();
        assert_eq!(
            n,
            A11yTarget::Node {
                node: "1.2:3".parse().unwrap()
            }
        );
        let json = serde_json::to_string(&A11yTarget::Desktop).unwrap();
        assert_eq!(json, r#"{"type":"desktop"}"#);
        let via: ClickVia = serde_json::from_str("\"pointer\"").unwrap();
        assert_eq!(via, ClickVia::Pointer);
    }
}
