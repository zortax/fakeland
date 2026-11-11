//! Node queries: role, name and text patterns, required and forbidden states, an action and an
//! interface. Every given condition must hold.

use regex::{Regex, RegexBuilder};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{A11yError, Node, actions, roles};

/// A node query. Every given field must match.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Query {
    /// The role. It matches the whole role name or its last words, so `button` matches
    /// `push button` and `toggle button`; toolkit names of one control match each other, so
    /// `entry` matches `text box`.
    pub role: Option<String>,
    /// A regular expression for the name. It matches case-insensitively anywhere in the name.
    pub name: Option<String>,
    /// A regular expression for the name, the description or the text contents.
    pub text: Option<String>,
    /// States that the node carries. A leading `!` requires the state to be absent.
    pub states: Vec<String>,
    /// An action that the node offers, for example `click`.
    pub action: Option<String>,
    /// An interface that the node implements, for example `EditableText`.
    pub interface: Option<String>,
}

impl Query {
    /// Returns a query for a role.
    pub fn role(role: impl Into<String>) -> Self {
        Self {
            role: Some(role.into()),
            ..Self::default()
        }
    }

    /// Adds a name pattern.
    pub fn with_name(mut self, pattern: impl Into<String>) -> Self {
        self.name = Some(pattern.into());
        self
    }

    /// Adds a text pattern.
    pub fn with_text(mut self, pattern: impl Into<String>) -> Self {
        self.text = Some(pattern.into());
        self
    }

    /// Adds a state condition.
    pub fn with_state(mut self, state: impl Into<String>) -> Self {
        self.states.push(state.into());
        self
    }

    /// Adds an action condition.
    pub fn with_action(mut self, action: impl Into<String>) -> Self {
        self.action = Some(action.into());
        self
    }

    /// Compiles the patterns.
    pub fn compile(&self) -> Result<Matcher, A11yError> {
        let pattern = |p: &Option<String>| -> Result<Option<Regex>, A11yError> {
            p.as_deref()
                .map(|p| RegexBuilder::new(p).case_insensitive(true).build())
                .transpose()
                .map_err(A11yError::from)
        };
        let mut required = Vec::new();
        let mut forbidden = Vec::new();
        for s in &self.states {
            match s.strip_prefix('!') {
                Some(rest) => forbidden.push(rest.trim().to_lowercase()),
                None => required.push(s.trim().to_lowercase()),
            }
        }
        Ok(Matcher {
            role: self.role.clone(),
            name: pattern(&self.name)?,
            text: pattern(&self.text)?,
            required,
            forbidden,
            action: self.action.clone(),
            interface: self.interface.clone(),
        })
    }

    /// Returns true when the query has no condition.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// A compiled query.
#[derive(Clone, Debug)]
pub struct Matcher {
    role: Option<String>,
    name: Option<Regex>,
    text: Option<Regex>,
    required: Vec<String>,
    forbidden: Vec<String>,
    action: Option<String>,
    interface: Option<String>,
}

impl Matcher {
    /// Returns true when a node matches.
    pub fn matches(&self, node: &Node) -> bool {
        if let Some(r) = &self.role
            && !roles::matches(&node.role, r)
        {
            return false;
        }
        if let Some(re) = &self.name
            && !re.is_match(&node.name)
        {
            return false;
        }
        if let Some(re) = &self.text {
            let text = node.text.as_deref().unwrap_or("");
            if !(re.is_match(&node.name) || re.is_match(&node.description) || re.is_match(text)) {
                return false;
            }
        }
        if !self.required.iter().all(|s| node.has_state(s)) {
            return false;
        }
        if self.forbidden.iter().any(|s| node.has_state(s)) {
            return false;
        }
        if let Some(a) = &self.action
            && actions::resolve(&node.actions, a).is_none()
        {
            return false;
        }
        if let Some(i) = &self.interface
            && !node.interfaces.iter().any(|x| x.eq_ignore_ascii_case(i))
        {
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeRef;

    fn button() -> Node {
        let mut n = Node::empty(NodeRef::new(":1.1", "/org/a11y/atspi/accessible/9"));
        n.role = "push button".into();
        n.name = "OK".into();
        n.description = "Close the dialog".into();
        n.states = vec!["enabled".into(), "focusable".into(), "showing".into()];
        n.interfaces = vec!["Accessible".into(), "Action".into(), "Component".into()];
        n.actions = vec!["click".into()];
        n
    }

    #[test]
    fn every_condition_must_hold() {
        let n = button();
        let q = Query::role("button")
            .with_name("^ok$")
            .with_state("showing");
        assert!(q.compile().unwrap().matches(&n));
        assert!(!Query::role("label").compile().unwrap().matches(&n));
        assert!(
            !Query::default()
                .with_name("cancel")
                .compile()
                .unwrap()
                .matches(&n)
        );
        assert!(Query::default().compile().unwrap().matches(&n));
    }

    #[test]
    fn states_can_be_forbidden() {
        let n = button();
        assert!(
            Query::default()
                .with_state("!focused")
                .compile()
                .unwrap()
                .matches(&n)
        );
        assert!(
            !Query::default()
                .with_state("!enabled")
                .compile()
                .unwrap()
                .matches(&n)
        );
        assert!(
            !Query::default()
                .with_state("checked")
                .compile()
                .unwrap()
                .matches(&n)
        );
    }

    #[test]
    fn text_matches_name_description_and_contents() {
        let mut n = button();
        assert!(
            Query::default()
                .with_text("dialog")
                .compile()
                .unwrap()
                .matches(&n)
        );
        n.text = Some("hello world".into());
        assert!(
            Query::default()
                .with_text("WORLD")
                .compile()
                .unwrap()
                .matches(&n)
        );
        assert!(
            !Query::default()
                .with_text("absent")
                .compile()
                .unwrap()
                .matches(&n)
        );
    }

    #[test]
    fn actions_and_interfaces() {
        let n = button();
        assert!(
            Query::default()
                .with_action("press")
                .compile()
                .unwrap()
                .matches(&n)
        );
        assert!(
            !Query::default()
                .with_action("expand")
                .compile()
                .unwrap()
                .matches(&n)
        );
        let q = Query {
            interface: Some("action".into()),
            ..Query::default()
        };
        assert!(q.compile().unwrap().matches(&n));
    }

    #[test]
    fn bad_patterns_fail_to_compile() {
        assert!(Query::default().with_name("(").compile().is_err());
    }

    #[test]
    fn serde_defaults() {
        let q: Query = serde_json::from_str(r#"{"role":"button"}"#).unwrap();
        assert_eq!(q, Query::role("button"));
        assert!(Query::default().is_empty());
    }
}
