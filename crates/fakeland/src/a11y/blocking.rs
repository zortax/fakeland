//! Blocking twins of the accessibility handle calls.

use fakeland_a11y::{AppInfo, Node, NodeRef, Query, Scope, TreeOptions, WindowFrame};

use super::{A11yTarget, Accessibility, ClickOutcome, ClickVia, TextMethod};
use crate::session::SessionError;
use crate::util::block_on;

impl Accessibility {
    /// Returns the windows on the calling thread.
    pub fn blocking_windows(&self) -> Result<Vec<WindowFrame>, SessionError> {
        block_on(self.windows())
    }

    /// Resolves a target on the calling thread.
    pub fn blocking_scope(
        &self,
        target: &A11yTarget,
    ) -> Result<(Scope, Vec<WindowFrame>), SessionError> {
        block_on(self.scope(target))
    }

    /// Lists the apps on the calling thread.
    pub fn blocking_apps(&self) -> Result<Vec<AppInfo>, SessionError> {
        block_on(self.apps())
    }

    /// Returns a tree on the calling thread.
    pub fn blocking_tree(
        &self,
        target: &A11yTarget,
        opts: &TreeOptions,
    ) -> Result<Node, SessionError> {
        block_on(self.tree(target, opts))
    }

    /// Searches on the calling thread.
    pub fn blocking_find(
        &self,
        target: &A11yTarget,
        query: &Query,
        opts: &TreeOptions,
        limit: usize,
    ) -> Result<Vec<Node>, SessionError> {
        block_on(self.find(target, query, opts, limit))
    }

    /// Returns one node on the calling thread.
    pub fn blocking_node(&self, node: &NodeRef) -> Result<Node, SessionError> {
        block_on(self.node(node))
    }

    /// Runs an action on the calling thread.
    pub fn blocking_act(&self, node: &NodeRef, action: &str) -> Result<String, SessionError> {
        block_on(self.act(node, action))
    }

    /// Clicks a node on the calling thread.
    pub fn blocking_click(
        &self,
        node: &NodeRef,
        via: ClickVia,
    ) -> Result<ClickOutcome, SessionError> {
        block_on(self.click(node, via))
    }

    /// Replaces text on the calling thread.
    pub fn blocking_set_text(
        &self,
        node: &NodeRef,
        text: &str,
    ) -> Result<TextMethod, SessionError> {
        block_on(self.set_text(node, text))
    }

    /// Sets a value on the calling thread.
    pub fn blocking_set_value(&self, node: &NodeRef, value: f64) -> Result<(), SessionError> {
        block_on(self.set_value(node, value))
    }

    /// Moves focus on the calling thread.
    pub fn blocking_focus(&self, node: &NodeRef) -> Result<(), SessionError> {
        block_on(self.focus(node))
    }
}
