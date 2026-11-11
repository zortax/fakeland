//! Accessibility methods: the tree of a window or app, node search, actions, text and focus.
//! Node ids in parameters are short names such as `n12` from an earlier result of the same
//! server, or full ids such as `1.23:42`.

use fakeland::accessibility::{AppInfo, Node, NodeRef, Query, TreeOptions};
use fakeland::{A11yTarget, ClickVia, SentInput, TextMethod};
use fakeland_types::{Logical, Point};
use serde::{Deserialize, Serialize};

use super::NoParams;
use super::method::method;

/// The limits of a tree walk on the wire.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct A11yLimits {
    /// The deepest level below the root.
    pub max_depth: usize,
    /// The most nodes.
    pub max_nodes: usize,
    /// Keep nodes without the `showing` state.
    pub all: bool,
    /// The most characters of text contents per node.
    pub text_limit: usize,
}

impl Default for A11yLimits {
    fn default() -> Self {
        Self {
            max_depth: 30,
            max_nodes: 1500,
            all: false,
            text_limit: 200,
        }
    }
}

impl A11yLimits {
    /// Returns the walk options.
    pub fn options(&self) -> TreeOptions {
        TreeOptions {
            max_depth: self.max_depth,
            max_nodes: self.max_nodes,
            showing_only: !self.all,
            text_limit: self.text_limit,
            ..TreeOptions::default()
        }
    }
}

/// A tree request.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct A11yTreeParams {
    /// The window, app or node. The default is the focused window.
    #[serde(default)]
    pub target: A11yTarget,
    /// A node as a short name or a full id. Its subtree replaces `target`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// The walk limits.
    #[serde(default, flatten)]
    pub limits: A11yLimits,
}

/// A tree with its text form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A11yTree {
    /// The root node.
    pub tree: Node,
    /// The compact text form with short node names.
    pub text: String,
    /// The number of fetched nodes.
    pub nodes: usize,
}

/// A search request.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A11yFindParams {
    /// The window, app or node to search. The default is the focused window.
    #[serde(default)]
    pub target: A11yTarget,
    /// A node as a short name or a full id. Its subtree replaces `target`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    /// The conditions.
    #[serde(default, flatten)]
    pub query: Query,
    /// The most results.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// The walk limits.
    #[serde(default, flatten)]
    pub limits: A11yLimits,
}

fn default_limit() -> usize {
    20
}

impl Default for A11yFindParams {
    fn default() -> Self {
        Self {
            target: A11yTarget::default(),
            root: None,
            query: Query::default(),
            limit: default_limit(),
            limits: A11yLimits::default(),
        }
    }
}

/// Search results with their text form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A11yFound {
    /// The matching nodes without children.
    pub nodes: Vec<Node>,
    /// One line per node with short node names.
    pub text: String,
}

/// A node request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A11yNodeParams {
    /// The node: a short name or a full id.
    pub node: String,
}

/// An action request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A11yActionParams {
    /// The node: a short name or a full id.
    pub node: String,
    /// The action. An activation name such as `click` clicks the node; any other name runs that
    /// action of the node.
    #[serde(default = "default_action")]
    pub action: String,
    /// How a click reaches the node.
    #[serde(default)]
    pub via: ClickVia,
}

fn default_action() -> String {
    "click".into()
}

/// What an action did.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct A11yActed {
    /// The node.
    pub node: NodeRef,
    /// The way a click reached the node, `None` for other actions.
    pub via: Option<ClickVia>,
    /// The action that ran.
    pub action: Option<String>,
    /// The global logical point of a pointer click.
    pub point: Option<Point<Logical>>,
    /// The pointer events of a pointer click.
    pub sent: Vec<SentInput>,
}

/// A text request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A11ySetTextParams {
    /// The node: a short name or a full id.
    pub node: String,
    /// The new text contents.
    pub text: String,
}

/// What a text change did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct A11yTextSet {
    /// How the text reached the node.
    pub method: TextMethod,
}

method!(
    /// Lists the accessible apps and their frames.
    A11yApps, "a11y_apps", NoParams => Vec<AppInfo>
);
method!(
    /// Returns the accessibility tree of a window, app or node.
    A11yTreeOf, "a11y_tree", A11yTreeParams => A11yTree
);
method!(
    /// Searches nodes by role, name, text, states, action and interface.
    A11yFind, "a11y_find", A11yFindParams => A11yFound
);
method!(
    /// Returns one node with its bounds.
    A11yNode, "a11y_node", A11yNodeParams => Node
);
method!(
    /// Clicks a node or runs one of its actions.
    A11yAction, "a11y_action", A11yActionParams => A11yActed
);
method!(
    /// Replaces the text of a node.
    A11ySetText, "a11y_set_text", A11ySetTextParams => A11yTextSet
);
method!(
    /// Moves keyboard focus inside the app to a node.
    A11yFocus, "a11y_focus", A11yNodeParams => ()
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_take_defaults() {
        let p: A11yTreeParams = serde_json::from_str("{}").unwrap();
        assert_eq!(p.target, A11yTarget::default());
        assert!(p.limits.options().showing_only);
        let f: A11yFindParams =
            serde_json::from_str(r#"{"role":"button","name":"ok","all":true,"limit":3}"#).unwrap();
        assert_eq!(f.query.role.as_deref(), Some("button"));
        assert_eq!(f.limit, 3);
        assert!(!f.limits.options().showing_only);
        let a: A11yActionParams = serde_json::from_str(r#"{"node":"n4"}"#).unwrap();
        assert_eq!(a.action, "click");
        assert_eq!(a.via, ClickVia::Auto);
    }
}
