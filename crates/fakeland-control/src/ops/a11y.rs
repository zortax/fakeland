//! Accessibility operations. The controller keeps one accessibility client, so short node names
//! stay valid across requests.

use fakeland::accessibility::{A11yError, AppInfo, Node, NodeRef, TextOptions, can_activate};
use fakeland::{A11yTarget, Accessibility, SessionError};

use super::{Controller, OpError};
use crate::methods::{
    A11yActed, A11yActionParams, A11yFindParams, A11yFound, A11yNodeParams, A11ySetTextParams,
    A11yTextSet, A11yTree, A11yTreeParams, NoParams,
};

impl Controller {
    /// Returns the accessibility handle and connects on first use.
    pub async fn accessibility(&self) -> Result<Accessibility, OpError> {
        if let Some(a) = self.a11y_slot().clone() {
            return Ok(a);
        }
        let a = self.session().a11y().await?;
        *self.a11y_slot() = Some(a.clone());
        Ok(a)
    }

    fn a11y_slot(&self) -> std::sync::MutexGuard<'_, Option<Accessibility>> {
        self.inner.a11y.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Drops the cached handle after a bus failure, so the next call connects again.
    fn a11y_failed<T>(&self, r: Result<T, SessionError>) -> Result<T, OpError> {
        if let Err(SessionError::A11y(A11yError::Bus(_))) = &r {
            *self.a11y_slot() = None;
        }
        Ok(r?)
    }

    /// Resolves a short name or a full node id.
    pub async fn a11y_node_ref(&self, id: &str) -> Result<NodeRef, OpError> {
        let a = self.accessibility().await?;
        a.client()
            .parse_id(id)
            .map_err(|e| OpError::InvalidParams(e.to_string()))
    }

    /// Returns the target, or the subtree of `root` when it is set.
    async fn a11y_target(
        &self,
        target: A11yTarget,
        root: Option<&str>,
    ) -> Result<A11yTarget, OpError> {
        match root {
            Some(id) => Ok(A11yTarget::Node {
                node: self.a11y_node_ref(id).await?,
            }),
            None => Ok(target),
        }
    }

    /// Lists the accessible apps.
    pub async fn a11y_apps(&self, _: NoParams) -> Result<Vec<AppInfo>, OpError> {
        let a = self.accessibility().await?;
        self.a11y_failed(a.apps().await)
    }

    /// Returns a tree with its text form.
    pub async fn a11y_tree(&self, p: A11yTreeParams) -> Result<A11yTree, OpError> {
        let target = self.a11y_target(p.target, p.root.as_deref()).await?;
        let a = self.accessibility().await?;
        let tree = self.a11y_failed(a.tree(&target, &p.limits.options()).await)?;
        let text = a.client().render(&tree, &TextOptions::default());
        Ok(A11yTree {
            nodes: tree.count(),
            text,
            tree,
        })
    }

    /// Searches nodes.
    pub async fn a11y_find(&self, p: A11yFindParams) -> Result<A11yFound, OpError> {
        p.query
            .compile()
            .map_err(|e| OpError::InvalidParams(e.to_string()))?;
        let target = self.a11y_target(p.target, p.root.as_deref()).await?;
        let a = self.accessibility().await?;
        let nodes = self.a11y_failed(
            a.find(&target, &p.query, &p.limits.options(), p.limit)
                .await,
        )?;
        let text = a.client().render_list(&nodes, &TextOptions::default());
        Ok(A11yFound { nodes, text })
    }

    /// Returns one node.
    pub async fn a11y_node(&self, p: A11yNodeParams) -> Result<Node, OpError> {
        let node = self.a11y_node_ref(&p.node).await?;
        let a = self.accessibility().await?;
        self.a11y_failed(a.node(&node).await)
    }

    /// Clicks a node or runs one of its actions.
    pub async fn a11y_action(&self, p: A11yActionParams) -> Result<A11yActed, OpError> {
        let node = self.a11y_node_ref(&p.node).await?;
        let a = self.accessibility().await?;
        if can_activate(std::slice::from_ref(&p.action)) {
            let out = self.a11y_failed(a.click(&node, p.via).await)?;
            return Ok(A11yActed {
                node,
                via: Some(out.via),
                action: out.action,
                point: out.point,
                sent: out.sent,
            });
        }
        let action = self.a11y_failed(a.act(&node, &p.action).await)?;
        Ok(A11yActed {
            node,
            via: None,
            action: Some(action),
            point: None,
            sent: Vec::new(),
        })
    }

    /// Replaces the text of a node.
    pub async fn a11y_set_text(&self, p: A11ySetTextParams) -> Result<A11yTextSet, OpError> {
        let node = self.a11y_node_ref(&p.node).await?;
        let a = self.accessibility().await?;
        let method = self.a11y_failed(a.set_text(&node, &p.text).await)?;
        Ok(A11yTextSet { method })
    }

    /// Moves keyboard focus inside the app to a node.
    pub async fn a11y_focus(&self, p: A11yNodeParams) -> Result<(), OpError> {
        let node = self.a11y_node_ref(&p.node).await?;
        let a = self.accessibility().await?;
        self.a11y_failed(a.focus(&node).await)
    }
}
