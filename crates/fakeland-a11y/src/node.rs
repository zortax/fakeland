//! The description of one accessible node and its subtree.

use fakeland_types::{Logical, Rect, WindowId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::NodeRef;

/// The coordinate space of reported extents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CoordSpace {
    /// Relative to the toplevel window of the app.
    Window,
    /// Screen coordinates as the app knows them. On Wayland an app does not know its position, so
    /// these are window-relative as well.
    Screen,
}

/// Extents as the app reports them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Extents {
    /// Left edge.
    pub x: i32,
    /// Top edge.
    pub y: i32,
    /// Width.
    pub w: i32,
    /// Height.
    pub h: i32,
    /// The coordinate space.
    pub space: CoordSpace,
}

impl Extents {
    /// The largest coordinate or length that a toolkit plausibly reports.
    pub const LIMIT: i32 = 1 << 20;

    /// Returns true for extents with a size of zero or more and coordinates within
    /// [`Extents::LIMIT`]. Toolkits send garbage extents for widgets that never got a size.
    pub fn is_plausible(&self) -> bool {
        let l = Self::LIMIT;
        (0..=l).contains(&self.w)
            && (0..=l).contains(&self.h)
            && (-l..=l).contains(&self.x)
            && (-l..=l).contains(&self.y)
    }
}

/// The range and current value of a node with the Value interface.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NodeValue {
    /// The current value.
    pub current: f64,
    /// The minimum value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// The maximum value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
}

/// One accessible node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    /// The node id.
    pub id: NodeRef,
    /// The AT-SPI role name, for example `push button`.
    pub role: String,
    /// The accessible name.
    pub name: String,
    /// The accessible description.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    /// The set states, for example `focused` or `checked`.
    pub states: Vec<String>,
    /// The AT-SPI interfaces without the `org.a11y.atspi.` prefix.
    pub interfaces: Vec<String>,
    /// The action names.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    /// The text contents of a node with the Text interface, cut at the text limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The value of a node with the Value interface.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<NodeValue>,
    /// The extents as the app reports them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extents: Option<Extents>,
    /// The bounds in the global logical space of the session. `None` when the node has no
    /// extents or its window is unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bounds: Option<Rect<Logical>>,
    /// The compositor window that shows the node.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowId>,
    /// The number of children the app reports.
    pub child_count: usize,
    /// The fetched children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
    /// Children that the depth, size or visibility limits left out.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub omitted: usize,
    /// The bus error of a node that does not answer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

impl Node {
    /// Returns an empty node for an id.
    pub fn empty(id: NodeRef) -> Self {
        Self {
            id,
            role: String::new(),
            name: String::new(),
            description: String::new(),
            states: Vec::new(),
            interfaces: Vec::new(),
            actions: Vec::new(),
            text: None,
            value: None,
            extents: None,
            bounds: None,
            window: None,
            child_count: 0,
            children: Vec::new(),
            omitted: 0,
            error: None,
        }
    }

    /// Returns true when the node has a state.
    pub fn has_state(&self, state: &str) -> bool {
        crate::states::has_state(&self.states, state)
    }

    /// Returns true when the node implements an interface, for example `Action`.
    pub fn implements(&self, interface: &str) -> bool {
        self.interfaces.iter().any(|i| i == interface)
    }

    /// Returns the number of nodes in the subtree, this node included.
    pub fn count(&self) -> usize {
        1 + self.children.iter().map(Node::count).sum::<usize>()
    }

    /// Visits the subtree in depth-first order.
    pub fn walk<'a>(&'a self, f: &mut impl FnMut(&'a Node, usize)) {
        self.walk_at(0, f);
    }

    fn walk_at<'a>(&'a self, depth: usize, f: &mut impl FnMut(&'a Node, usize)) {
        f(self, depth);
        for c in &self.children {
            c.walk_at(depth + 1, f);
        }
    }

    /// Returns the first node of the subtree with an id.
    pub fn find_id(&self, id: &NodeRef) -> Option<&Node> {
        if &self.id == id {
            return Some(self);
        }
        self.children.iter().find_map(|c| c.find_id(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(path: &str) -> Node {
        Node::empty(NodeRef::new(
            ":1.1",
            format!("/org/a11y/atspi/accessible/{path}"),
        ))
    }

    #[test]
    fn walk_count_and_find() {
        let mut root = leaf("1");
        let mut mid = leaf("2");
        mid.children.push(leaf("3"));
        root.children.push(mid);
        root.children.push(leaf("4"));
        assert_eq!(root.count(), 4);
        let mut seen = Vec::new();
        root.walk(&mut |n, d| seen.push((n.id.to_string(), d)));
        assert_eq!(
            seen,
            vec![
                ("1.1:1".to_string(), 0),
                ("1.1:2".to_string(), 1),
                ("1.1:3".to_string(), 2),
                ("1.1:4".to_string(), 1)
            ]
        );
        let three = "1.1:3".parse().unwrap();
        assert_eq!(root.find_id(&three).unwrap().id, three);
    }

    #[test]
    fn garbage_extents_are_implausible() {
        let e = |x, y, w, h| Extents {
            x,
            y,
            w,
            h,
            space: CoordSpace::Window,
        };
        assert!(e(0, 0, 10, 10).is_plausible());
        assert!(e(-20, 5, 0, 0).is_plausible());
        assert!(!e(0, 0, -1059249632, 32670).is_plausible());
        assert!(!e(0, 0, 10, -1).is_plausible());
        assert!(!e(i32::MIN, 0, 10, 10).is_plausible());
    }

    #[test]
    fn json_skips_empty_fields() {
        let n = leaf("5");
        let json = serde_json::to_value(&n).unwrap();
        let mut keys: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(|k| k.as_str())
            .collect();
        keys.sort_unstable();
        assert_eq!(
            keys,
            vec!["child_count", "id", "interfaces", "name", "role", "states"]
        );
        let back: Node = serde_json::from_value(json).unwrap();
        assert_eq!(back, n);
    }
}
