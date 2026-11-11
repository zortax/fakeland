//! Parameters of the accessibility tools. Node ids are the short names such as `n12` that the
//! tree and search results print.

use schemars::JsonSchema;
use serde::Deserialize;

/// Selects the part of the accessibility tree a tool reads.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct A11yScopeParams {
    /// A window: "focused" (the default), an id like "3", "app:APP_ID" or "title:REGEX".
    #[serde(default)]
    pub window: Option<String>,
    /// An app instead of a window: a pid or a part of the accessible app name.
    #[serde(default)]
    pub app: Option<String>,
    /// A node instead of a window: its subtree.
    #[serde(default)]
    pub node: Option<String>,
    /// Keep nodes that are not showing on screen.
    #[serde(default)]
    pub all: bool,
}

/// An accessibility tree request.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct A11yTreeParams {
    /// What to read.
    #[serde(flatten)]
    pub scope: A11yScopeParams,
    /// The deepest level below the root.
    #[serde(default = "max_depth")]
    pub max_depth: usize,
    /// The most nodes.
    #[serde(default = "max_nodes")]
    pub max_nodes: usize,
    /// Also return the tree as JSON.
    #[serde(default)]
    pub json: bool,
}

/// A node search.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct A11yFindParams {
    /// Where to search.
    #[serde(flatten)]
    pub scope: A11yScopeParams,
    /// The role, for example "button", "push button", "entry", "check box", "link", "menu item".
    #[serde(default)]
    pub role: Option<String>,
    /// A case-insensitive regular expression for the accessible name.
    #[serde(default)]
    pub name: Option<String>,
    /// A case-insensitive regular expression for the name, description or text contents.
    #[serde(default)]
    pub text: Option<String>,
    /// Required states such as "focused", "checked", "showing"; "!state" requires absence.
    #[serde(default)]
    pub states: Vec<String>,
    /// An action the node offers, for example "click".
    #[serde(default)]
    pub action: Option<String>,
    /// The most results.
    #[serde(default = "find_limit")]
    pub limit: usize,
}

/// A node action.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct A11yActionParams {
    /// The node id from a tree or search result, for example "n12".
    pub node: String,
    /// The action. "click" (the default) activates the node; other names run that action.
    #[serde(default)]
    pub action: Option<String>,
    /// How a click reaches the node: "auto" (its action, else the pointer), "action" or
    /// "pointer" (a real click at the center of its bounds).
    #[serde(default)]
    pub via: Option<String>,
    /// Return a screenshot after the action.
    #[serde(default)]
    pub screenshot: bool,
}

/// A text change.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct A11ySetTextParams {
    /// The node id of an entry or text field, for example "n12".
    pub node: String,
    /// The new text contents.
    pub text: String,
    /// Return a screenshot after the change.
    #[serde(default)]
    pub screenshot: bool,
}

fn max_depth() -> usize {
    30
}

fn max_nodes() -> usize {
    800
}

fn find_limit() -> usize {
    20
}
