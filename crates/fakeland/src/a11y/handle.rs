//! The accessibility handle of a session: the AT-SPI client plus the session, so calls see the
//! current windows, resolve window references and fall back to pointer and keyboard input.

use std::sync::Weak;

use fakeland_a11y::{
    A11y, A11yError, AppInfo, Node, NodeRef, Query, Scope, TreeOptions, WindowFrame,
};
use fakeland_scenario::{Button, TypingCadence};
use fakeland_types::{Logical, Point};
use serde::{Deserialize, Serialize};

use super::{A11yTarget, ClickVia, TextMethod};
use crate::input::SentInput;
use crate::session::{Session, SessionError, SessionInner};

/// What a click did.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClickOutcome {
    /// The way the click reached the node.
    pub via: ClickVia,
    /// The action that ran.
    pub action: Option<String>,
    /// The global logical point of a pointer click.
    pub point: Option<Point<Logical>>,
    /// The pointer events of a pointer click.
    pub sent: Vec<SentInput>,
}

/// The accessibility tree of a session. Clones share one bus connection. The handle keeps no
/// session alive; calls after shutdown fail with [`SessionError::Closed`].
#[derive(Clone, Debug)]
pub struct Accessibility {
    session: Weak<SessionInner>,
    client: A11y,
}

impl Accessibility {
    pub(crate) fn new(session: Weak<SessionInner>, client: A11y) -> Self {
        Self { session, client }
    }

    /// Returns the AT-SPI client.
    pub fn client(&self) -> &A11y {
        &self.client
    }

    fn session(&self) -> Result<Session, SessionError> {
        self.session
            .upgrade()
            .map(Session::from_inner)
            .ok_or(SessionError::Closed)
    }

    /// Returns the mapped windows as accessibility frames.
    pub async fn windows(&self) -> Result<Vec<WindowFrame>, SessionError> {
        self.session()?
            .inner
            .call(|state| state.a11y_windows())
            .await
    }

    /// Resolves a target to a scope and the current windows.
    pub async fn scope(
        &self,
        target: &A11yTarget,
    ) -> Result<(Scope, Vec<WindowFrame>), SessionError> {
        let session = self.session()?;
        let wanted = target.clone();
        let (resolved, windows) = session
            .inner
            .call(move |state| {
                let id = match &wanted {
                    A11yTarget::Window { window } => Some(state.resolve_window(window)),
                    _ => None,
                };
                (id, state.a11y_windows())
            })
            .await?;
        let scope = match (target.clone(), resolved) {
            (A11yTarget::Window { .. }, Some(id)) => Scope::Window {
                window: id?.ok_or(SessionError::NoWindow)?,
            },
            (A11yTarget::Pid { pid }, _) => Scope::Pid { pid },
            (A11yTarget::App { name }, _) => Scope::AppName { name },
            (A11yTarget::Node { node }, _) => Scope::Node { node },
            _ => Scope::Desktop,
        };
        Ok((scope, windows))
    }

    /// Lists the accessible apps with their frames paired to windows.
    pub async fn apps(&self) -> Result<Vec<AppInfo>, SessionError> {
        let windows = self.windows().await?;
        Ok(self.client.apps(&windows).await?)
    }

    /// Returns the tree of a target with bounds in the global logical space.
    pub async fn tree(
        &self,
        target: &A11yTarget,
        opts: &TreeOptions,
    ) -> Result<Node, SessionError> {
        let (scope, windows) = self.scope(target).await?;
        Ok(self.client.tree(&scope, &windows, opts).await?)
    }

    /// Returns at most `limit` nodes of a target that match a query.
    pub async fn find(
        &self,
        target: &A11yTarget,
        query: &Query,
        opts: &TreeOptions,
        limit: usize,
    ) -> Result<Vec<Node>, SessionError> {
        let (scope, windows) = self.scope(target).await?;
        Ok(self
            .client
            .find(&scope, query, &windows, opts, limit)
            .await?)
    }

    /// Returns one node with its bounds.
    pub async fn node(&self, node: &NodeRef) -> Result<Node, SessionError> {
        let windows = self.windows().await?;
        Ok(self.client.node(node, &windows).await?)
    }

    /// Runs an action of a node and returns the name of the action that ran.
    pub async fn act(&self, node: &NodeRef, action: &str) -> Result<String, SessionError> {
        Ok(self.client.act(node, action).await?)
    }

    /// Clicks a node through its activation action or with the pointer at its center.
    pub async fn click(&self, node: &NodeRef, via: ClickVia) -> Result<ClickOutcome, SessionError> {
        if via != ClickVia::Pointer {
            match self.client.act(node, "click").await {
                Ok(action) => {
                    return Ok(ClickOutcome {
                        via: ClickVia::Action,
                        action: Some(action),
                        point: None,
                        sent: Vec::new(),
                    });
                }
                Err(e) if via == ClickVia::Action => return Err(e.into()),
                Err(e) => tracing::debug!("no activation action, the pointer clicks: {e}"),
            }
        }
        let (point, sent) = self.pointer_click(node).await?;
        Ok(ClickOutcome {
            via: ClickVia::Pointer,
            action: None,
            point: Some(point),
            sent,
        })
    }

    /// Clicks the left button at the center of the node bounds.
    async fn pointer_click(
        &self,
        node: &NodeRef,
    ) -> Result<(Point<Logical>, Vec<SentInput>), SessionError> {
        let n = self.node(node).await?;
        let bounds = n
            .bounds
            .filter(|b| !b.is_empty())
            .ok_or_else(|| A11yError::NoBounds(node.clone()))?;
        let center = bounds.center();
        let session = self.session()?;
        let window = n.window;
        let pos = session
            .inner
            .call(move |state| {
                let fallback = window.and_then(|w| state.windows.get(w)).map(|r| r.output);
                state.global_to_position(center, fallback)
            })
            .await?;
        let sent = session.click(pos, Button::Left).await?;
        Ok((center, sent))
    }

    /// Replaces the text of a node. A node without the EditableText interface takes focus, gets
    /// `ctrl+a` and then the typed text.
    pub async fn set_text(&self, node: &NodeRef, text: &str) -> Result<TextMethod, SessionError> {
        match self.client.set_text(node, text).await {
            Ok(()) => return Ok(TextMethod::EditableText),
            Err(A11yError::Unsupported { .. } | A11yError::Refused { .. }) => {}
            Err(e) => return Err(e.into()),
        }
        if self.client.focus(node).await.is_err() {
            self.pointer_click(node).await?;
        }
        let session = self.session()?;
        session.combo("ctrl+a").await?;
        if text.is_empty() {
            session.key("BackSpace").await?;
        } else {
            session.type_text(text, TypingCadence::default()).await?;
        }
        Ok(TextMethod::Typed)
    }

    /// Sets the value of a node with the Value interface.
    pub async fn set_value(&self, node: &NodeRef, value: f64) -> Result<(), SessionError> {
        Ok(self.client.set_value(node, value).await?)
    }

    /// Moves keyboard focus inside the app to a node.
    pub async fn focus(&self, node: &NodeRef) -> Result<(), SessionError> {
        Ok(self.client.focus(node).await?)
    }
}
