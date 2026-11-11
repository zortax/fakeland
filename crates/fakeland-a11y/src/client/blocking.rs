//! Blocking twins of the client calls. Each runs its async call on the calling thread.

use std::time::Duration;

use super::{A11y, AppInfo};
use crate::block_on::block_on;
use crate::{A11yError, Node, NodeRef, Query, Scope, TreeOptions, WindowFrame};

impl A11y {
    /// Connects on the calling thread. See [`A11y::connect`].
    pub fn blocking_connect(session_bus: &str) -> Result<Self, A11yError> {
        block_on(Self::connect(session_bus))
    }

    /// Connects to an accessibility bus address on the calling thread.
    pub fn blocking_connect_bus(address: &str, timeout: Duration) -> Result<Self, A11yError> {
        block_on(Self::connect_bus(address, timeout))
    }

    /// Lists the apps on the calling thread. See [`A11y::apps`].
    pub fn blocking_apps(&self, windows: &[WindowFrame]) -> Result<Vec<AppInfo>, A11yError> {
        block_on(self.apps(windows))
    }

    /// Returns a tree on the calling thread. See [`A11y::tree`].
    pub fn blocking_tree(
        &self,
        scope: &Scope,
        windows: &[WindowFrame],
        opts: &TreeOptions,
    ) -> Result<Node, A11yError> {
        block_on(self.tree(scope, windows, opts))
    }

    /// Searches on the calling thread. See [`A11y::find`].
    pub fn blocking_find(
        &self,
        scope: &Scope,
        query: &Query,
        windows: &[WindowFrame],
        opts: &TreeOptions,
        limit: usize,
    ) -> Result<Vec<Node>, A11yError> {
        block_on(self.find(scope, query, windows, opts, limit))
    }

    /// Returns one node on the calling thread. See [`A11y::node`].
    pub fn blocking_node(
        &self,
        node: &NodeRef,
        windows: &[WindowFrame],
    ) -> Result<Node, A11yError> {
        block_on(self.node(node, windows))
    }

    /// Runs an action on the calling thread. See [`A11y::act`].
    pub fn blocking_act(&self, node: &NodeRef, action: &str) -> Result<String, A11yError> {
        block_on(self.act(node, action))
    }

    /// Replaces text on the calling thread. See [`A11y::set_text`].
    pub fn blocking_set_text(&self, node: &NodeRef, text: &str) -> Result<(), A11yError> {
        block_on(self.set_text(node, text))
    }

    /// Sets a value on the calling thread. See [`A11y::set_value`].
    pub fn blocking_set_value(&self, node: &NodeRef, value: f64) -> Result<(), A11yError> {
        block_on(self.set_value(node, value))
    }

    /// Moves focus on the calling thread. See [`A11y::focus`].
    pub fn blocking_focus(&self, node: &NodeRef) -> Result<(), A11yError> {
        block_on(self.focus(node))
    }

    /// Starts the registry on the calling thread. See [`A11y::start_registry`].
    pub fn blocking_start_registry(&self) -> Result<(), A11yError> {
        block_on(self.start_registry())
    }
}
