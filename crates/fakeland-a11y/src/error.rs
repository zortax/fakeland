//! Errors of accessibility calls.

use thiserror::Error;

use crate::NodeRef;

/// An accessibility call that failed.
#[derive(Debug, Error)]
pub enum A11yError {
    /// The session bus has no `org.a11y.Bus` service.
    #[error("no accessibility bus on the session bus: {0}")]
    NoBus(String),
    /// A D-Bus call failed.
    #[error("d-bus: {0}")]
    Bus(#[from] zbus::Error),
    /// A node id does not parse.
    #[error("invalid node id {0:?}")]
    InvalidId(String),
    /// No node has this short name. Short names come from rendered trees and searches.
    #[error("no node has the short name {0}; read the tree or search again")]
    UnknownName(String),
    /// A name pattern does not compile.
    #[error("invalid pattern: {0}")]
    Pattern(#[from] regex::Error),
    /// No accessible app matches.
    #[error("no accessible app matches {0}")]
    NoApp(String),
    /// The app of a window exposes no matching frame.
    #[error("no accessible frame for window {0}")]
    NoFrame(String),
    /// The node is gone or does not answer.
    #[error("node {node} is not reachable: {message}")]
    Node {
        /// The node.
        node: NodeRef,
        /// The bus error text.
        message: String,
    },
    /// The node has no action of that name.
    #[error("node {node} has no action {action:?}; actions: {available:?}")]
    NoAction {
        /// The node.
        node: NodeRef,
        /// The requested action.
        action: String,
        /// The actions of the node.
        available: Vec<String>,
    },
    /// The node lacks the interface that the call needs.
    #[error("node {node} does not implement {interface}")]
    Unsupported {
        /// The node.
        node: NodeRef,
        /// The missing interface.
        interface: &'static str,
    },
    /// The app refused the request.
    #[error("node {node} refused {request}")]
    Refused {
        /// The node.
        node: NodeRef,
        /// The request.
        request: String,
    },
    /// The node has no bounds.
    #[error("node {0} has no screen bounds")]
    NoBounds(NodeRef),
}
