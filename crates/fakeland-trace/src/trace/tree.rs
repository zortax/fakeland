//! The surface tree: parents of subsurfaces and popups, and the toplevel surface of each window.

use std::collections::{BTreeSet, HashMap};

use fakeland_types::{SurfaceId, WindowId};

use crate::event::{Event, Record};

/// Parent links between surfaces and the toplevel surface of each window.
#[derive(Clone, Debug, Default)]
pub struct SurfaceTree {
    parents: HashMap<SurfaceId, SurfaceId>,
    windows: HashMap<WindowId, SurfaceId>,
}

impl SurfaceTree {
    /// Builds the tree from `SurfaceRole` and `WindowMapped` records.
    pub fn from_records<'a>(records: impl IntoIterator<Item = &'a Record>) -> Self {
        let mut tree = Self::default();
        for r in records {
            tree.observe(&r.event);
        }
        tree
    }

    /// Adds the links of one event.
    pub fn observe(&mut self, event: &Event) {
        match event {
            Event::SurfaceRole {
                surface,
                parent: Some(parent),
                ..
            } if surface != parent => {
                self.parents.insert(*surface, *parent);
            }
            Event::WindowMapped {
                window, surface, ..
            } => {
                self.windows.insert(*window, *surface);
            }
            _ => {}
        }
    }

    /// Returns the parent of a surface.
    pub fn parent(&self, surface: SurfaceId) -> Option<SurfaceId> {
        self.parents.get(&surface).copied()
    }

    /// Returns the topmost ancestor of a surface, or the surface itself when it has no parent.
    pub fn root(&self, surface: SurfaceId) -> SurfaceId {
        let mut s = surface;
        // The bound stops a cycle in a malformed trace.
        for _ in 0..=self.parents.len() {
            match self.parents.get(&s) {
                Some(p) => s = *p,
                None => break,
            }
        }
        s
    }

    /// Returns the direct children of a surface, in id order.
    pub fn children(&self, surface: SurfaceId) -> Vec<SurfaceId> {
        let mut c: Vec<SurfaceId> = self
            .parents
            .iter()
            .filter(|(_, p)| **p == surface)
            .map(|(s, _)| *s)
            .collect();
        c.sort();
        c
    }

    /// Returns a surface and all its descendants.
    pub fn subtree(&self, surface: SurfaceId) -> BTreeSet<SurfaceId> {
        let mut out = BTreeSet::from([surface]);
        let mut stack = vec![surface];
        while let Some(s) = stack.pop() {
            for c in self.children(s) {
                if out.insert(c) {
                    stack.push(c);
                }
            }
        }
        out
    }

    /// Returns the toplevel surface of a window.
    pub fn window_surface(&self, window: WindowId) -> Option<SurfaceId> {
        self.windows.get(&window).copied()
    }

    /// Returns the toplevel surface and all its descendants of a window.
    pub fn window_surfaces(&self, window: WindowId) -> BTreeSet<SurfaceId> {
        self.window_surface(window)
            .map(|s| self.subtree(s))
            .unwrap_or_default()
    }

    /// Returns the window whose surface tree holds a surface.
    pub fn window_of(&self, surface: SurfaceId) -> Option<WindowId> {
        let root = self.root(surface);
        let mut found: Vec<WindowId> = self
            .windows
            .iter()
            .filter(|(_, s)| **s == root)
            .map(|(w, _)| *w)
            .collect();
        found.sort();
        found.pop()
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::Timestamp;

    use super::*;

    fn role(s: u64, parent: Option<u64>) -> Record {
        Record::new(
            Timestamp(0),
            Event::SurfaceRole {
                surface: SurfaceId(s),
                role: "subsurface".into(),
                parent: parent.map(SurfaceId),
            },
        )
    }

    #[test]
    fn subtree_and_root() {
        let records = vec![
            role(1, None),
            role(2, Some(1)),
            role(3, Some(2)),
            role(4, Some(1)),
            Record::new(
                Timestamp(0),
                Event::WindowMapped {
                    window: WindowId(7),
                    surface: SurfaceId(1),
                    app_id: "a".into(),
                    title: "t".into(),
                    output: None,
                },
            ),
        ];
        let t = SurfaceTree::from_records(&records);
        assert_eq!(t.root(SurfaceId(3)), SurfaceId(1));
        assert_eq!(t.children(SurfaceId(1)), vec![SurfaceId(2), SurfaceId(4)]);
        assert_eq!(t.window_surfaces(WindowId(7)).len(), 4);
        assert_eq!(t.window_of(SurfaceId(3)), Some(WindowId(7)));
        assert_eq!(t.window_of(SurfaceId(9)), None);
    }
}
