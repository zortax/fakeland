//! Short node names for agents. A client hands out `n1`, `n2`, … in the order it first renders
//! nodes, and the same node keeps its name for the life of the client. The table forgets every
//! name once it holds [`MAX_ALIASES`] nodes.

use std::collections::HashMap;

use crate::{A11yError, NodeRef};

/// The most names one table keeps.
pub const MAX_ALIASES: usize = 200_000;

/// A table of short node names.
#[derive(Debug, Default)]
pub struct Aliases {
    by_ref: HashMap<NodeRef, usize>,
    refs: Vec<NodeRef>,
}

impl Aliases {
    /// Returns the short name of a node and assigns one on first use.
    pub fn alias(&mut self, node: &NodeRef) -> String {
        if let Some(&i) = self.by_ref.get(node) {
            return format!("n{i}");
        }
        if self.refs.len() >= MAX_ALIASES {
            self.by_ref.clear();
            self.refs.clear();
        }
        self.refs.push(node.clone());
        let i = self.refs.len();
        self.by_ref.insert(node.clone(), i);
        format!("n{i}")
    }

    /// Resolves a short name or a full node id.
    pub fn resolve(&self, id: &str) -> Result<NodeRef, A11yError> {
        let id = id.trim().trim_start_matches('#');
        if let Some(n) = id.strip_prefix('n')
            && let Ok(i) = n.parse::<usize>()
        {
            return i
                .checked_sub(1)
                .and_then(|k| self.refs.get(k))
                .cloned()
                .ok_or_else(|| A11yError::UnknownName(id.to_string()));
        }
        id.parse()
    }

    /// Returns the number of named nodes.
    pub fn len(&self) -> usize {
        self.refs.len()
    }

    /// Returns true when no node has a name.
    pub fn is_empty(&self) -> bool {
        self.refs.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_stable_and_resolve() {
        let mut t = Aliases::default();
        let a = NodeRef::new(":1.2", "/org/gnome/Zenity/a11y/0a9e");
        let b = NodeRef::new(":1.2", "/org/a11y/atspi/accessible/7");
        assert_eq!(t.alias(&a), "n1");
        assert_eq!(t.alias(&b), "n2");
        assert_eq!(t.alias(&a), "n1");
        assert_eq!(t.resolve("n1").unwrap(), a);
        assert_eq!(t.resolve("#n2").unwrap(), b);
        assert_eq!(t.resolve("1.2:7").unwrap(), b);
        assert!(t.resolve("n3").is_err());
        assert!(t.resolve("n0").is_err());
        assert_eq!(t.len(), 2);
    }
}
