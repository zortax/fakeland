//! Requests on one node: run an action, replace the text, set the value and take focus.

use atspi::proxy::action::ActionProxy;
use atspi::proxy::component::ComponentProxy;
use atspi::proxy::editable_text::EditableTextProxy;
use atspi::proxy::value::ValueProxy;

use super::A11y;
use super::proxy::{accessible, proxy};
use crate::{A11yError, NodeRef, actions};

impl A11y {
    /// Runs the action that `action` names and returns the name of the action that ran. The
    /// activation names stand in for each other, so `click` also runs `press` or `activate`.
    pub async fn act(&self, node: &NodeRef, action: &str) -> Result<String, A11yError> {
        self.require(node, "Action").await?;
        let a: ActionProxy<'_> = proxy(&self.conn, node).await?;
        let names = super::fetch::actions(&self.conn, node)
            .await
            .ok_or_else(|| A11yError::Node {
                node: node.clone(),
                message: "the action names are not readable".into(),
            })?;
        let index = actions::resolve(&names, action).ok_or_else(|| A11yError::NoAction {
            node: node.clone(),
            action: action.to_string(),
            available: names.clone(),
        })?;
        let done = a
            .do_action(index as i32)
            .await
            .map_err(|e| unreachable_node(node, e))?;
        if !done {
            return Err(A11yError::Refused {
                node: node.clone(),
                request: format!("action {:?}", names[index]),
            });
        }
        Ok(names[index].clone())
    }

    /// Replaces the text contents of an editable node.
    pub async fn set_text(&self, node: &NodeRef, text: &str) -> Result<(), A11yError> {
        self.require(node, "EditableText").await?;
        let e: EditableTextProxy<'_> = proxy(&self.conn, node).await?;
        let done = e
            .set_text_contents(text)
            .await
            .map_err(|e| unreachable_node(node, e))?;
        if !done {
            return Err(A11yError::Refused {
                node: node.clone(),
                request: "set_text".into(),
            });
        }
        Ok(())
    }

    /// Sets the current value of a node with the Value interface.
    pub async fn set_value(&self, node: &NodeRef, value: f64) -> Result<(), A11yError> {
        self.require(node, "Value").await?;
        let v: ValueProxy<'_> = proxy(&self.conn, node).await?;
        v.set_current_value(value)
            .await
            .map_err(|e| unreachable_node(node, e))
    }

    /// Moves keyboard focus inside the app to a node.
    pub async fn focus(&self, node: &NodeRef) -> Result<(), A11yError> {
        self.require(node, "Component").await?;
        let c: ComponentProxy<'_> = proxy(&self.conn, node).await?;
        let done = c
            .grab_focus()
            .await
            .map_err(|e| unreachable_node(node, e))?;
        if !done {
            return Err(A11yError::Refused {
                node: node.clone(),
                request: "focus".into(),
            });
        }
        Ok(())
    }

    /// Checks that a node implements an interface.
    async fn require(&self, node: &NodeRef, interface: &'static str) -> Result<(), A11yError> {
        let acc = accessible(&self.conn, node).await?;
        let list = acc
            .get_interfaces()
            .await
            .map_err(|e| unreachable_node(node, e))?;
        let full = format!("org.a11y.atspi.{interface}");
        if list.contains(&full) {
            Ok(())
        } else {
            Err(A11yError::Unsupported {
                node: node.clone(),
                interface,
            })
        }
    }
}

fn unreachable_node(node: &NodeRef, e: zbus::Error) -> A11yError {
    A11yError::Node {
        node: node.clone(),
        message: e.to_string(),
    }
}
