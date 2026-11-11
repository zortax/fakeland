//! The place of a single node: its frame ancestor, the window of that frame, and the mapping
//! along the ancestor chain from the frame down to the node.

use futures_util::future::join_all;
use futures_util::join;

use super::A11y;
use super::fetch::extents;
use super::proxy::{accessible, node_ref, role_name};
use super::walk::Place;
use crate::{CoordSpace, Extents, Mapping, NodeRef, WindowFrame, roles};

/// The most parent steps from a node to its app root.
const MAX_ANCESTORS: usize = 256;

/// The ancestors of a node up to its frame.
struct Ancestry {
    /// The frame first, then each node below it down to the parent of the node.
    chain: Vec<NodeRef>,
    /// The app root.
    app: NodeRef,
}

impl A11y {
    /// Returns the place of a node and the extents of its parent.
    pub(crate) async fn place_of(
        &self,
        node: &NodeRef,
        windows: &[WindowFrame],
    ) -> (Place, Option<Extents>) {
        let Some(Ancestry { chain, app }) = self.ancestry(node).await else {
            return (Place::Unknown, None);
        };
        let frame = chain.first().unwrap_or(node).clone();
        let pid = self.pid_of(&app.bus).await;
        let Ok(acc) = accessible(&self.conn, &app).await else {
            return (Place::Unknown, None);
        };
        let children = acc
            .get_children()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(node_ref)
            .collect();
        let frames = self.frames(pid, children, windows).await;
        let Some(window) = frames
            .iter()
            .find(|f| f.node == frame)
            .and_then(|f| f.window)
            .and_then(|id| windows.iter().find(|w| w.id == id))
        else {
            return (Place::Unknown, None);
        };
        if chain.is_empty() {
            return (Place::Frame(window.clone()), None);
        }
        let ext = join_all(
            chain
                .iter()
                .map(|n| extents(&self.conn, n, CoordSpace::Window)),
        )
        .await;
        let space = ext[0].map(|e| e.space).unwrap_or(CoordSpace::Window);
        let mut m = Mapping::calibrate(ext[0], window, space);
        for pair in ext.windows(2) {
            m = m.descend(pair[0], pair[1]);
        }
        (Place::Inside(m), ext.last().copied().flatten())
    }

    /// Walks up from a node to the toplevel below its app root. The chain is empty when the node
    /// is the frame itself.
    async fn ancestry(&self, node: &NodeRef) -> Option<Ancestry> {
        let mut up = Vec::new();
        let mut current = node.clone();
        for _ in 0..MAX_ANCESTORS {
            let acc = accessible(&self.conn, &current).await.ok()?;
            let parent = node_ref(acc.parent().await.ok()?);
            if parent.is_null() || parent.bus == crate::id::REGISTRY {
                return None;
            }
            let pacc = accessible(&self.conn, &parent).await.ok()?;
            let (role, grand) = join!(role_name(&pacc), pacc.parent());
            let role = role.ok()?;
            let top = grand
                .ok()
                .map(node_ref)
                .is_none_or(|g| g.is_null() || g.bus == crate::id::REGISTRY);
            if roles::normalize(&role) == "application" || top {
                if &current != node {
                    up.push(current);
                }
                up.reverse();
                return Some(Ancestry {
                    chain: up,
                    app: parent,
                });
            }
            if &current != node {
                up.push(current);
            }
            current = parent;
        }
        None
    }
}
