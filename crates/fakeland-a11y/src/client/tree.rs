//! Tree and search calls: the scope resolves to a root node and its place, then the walk fetches
//! the subtree.

use futures_util::future::join_all;

use super::A11y;
use super::proxy::accessible;
use super::walk::Place;
use crate::{
    A11yError, CoordSpace, Extents, Node, NodeRef, Query, Scope, TreeOptions, WindowFrame,
};

impl A11y {
    /// Returns the tree of a scope. `windows` lists the compositor windows; the walk pairs
    /// frames with them and maps bounds into the global logical space.
    pub async fn tree(
        &self,
        scope: &Scope,
        windows: &[WindowFrame],
        opts: &TreeOptions,
    ) -> Result<Node, A11yError> {
        let (root, place, parent) = self.resolve(scope, windows).await?;
        Ok(self
            .walk(root, place, CoordSpace::Window, parent, windows, opts)
            .await)
    }

    /// Returns the nodes of a scope that match a query, in depth-first order, at most `limit`.
    /// The returned nodes carry no children.
    pub async fn find(
        &self,
        scope: &Scope,
        query: &Query,
        windows: &[WindowFrame],
        opts: &TreeOptions,
        limit: usize,
    ) -> Result<Vec<Node>, A11yError> {
        let matcher = query.compile()?;
        let tree = self.tree(scope, windows, opts).await?;
        let mut found = Vec::new();
        tree.walk(&mut |n, _| {
            if found.len() < limit && n.error.is_none() && matcher.matches(n) {
                let mut hit = n.clone();
                hit.children.clear();
                hit.omitted = 0;
                found.push(hit);
            }
        });
        Ok(found)
    }

    /// Returns one node without children, with bounds when its window is known.
    pub async fn node(&self, node: &NodeRef, windows: &[WindowFrame]) -> Result<Node, A11yError> {
        let scope = Scope::Node { node: node.clone() };
        let mut n = self
            .tree(&scope, windows, &TreeOptions::default().depth(0))
            .await?;
        if let Some(e) = n.error.take() {
            return Err(A11yError::Node {
                node: node.clone(),
                message: e,
            });
        }
        n.omitted = 0;
        Ok(n)
    }

    /// Resolves a scope to its root node and place.
    async fn resolve(
        &self,
        scope: &Scope,
        windows: &[WindowFrame],
    ) -> Result<(NodeRef, Place, Option<Extents>), A11yError> {
        match scope {
            Scope::Desktop => Ok((NodeRef::registry_root(), Place::Unknown, None)),
            Scope::Pid { pid } => {
                let roots = self.app_roots().await?;
                let pids = join_all(roots.iter().map(|r| self.pid_of(&r.bus))).await;
                roots
                    .into_iter()
                    .zip(pids)
                    .find(|(_, p)| *p == Some(*pid))
                    .map(|(r, _)| (r, Place::Unknown, None))
                    .ok_or_else(|| A11yError::NoApp(format!("pid {}", pid.0)))
            }
            Scope::AppName { name } => {
                let wanted = name.to_lowercase();
                for r in self.app_roots().await? {
                    let Ok(acc) = accessible(&self.conn, &r).await else {
                        continue;
                    };
                    if acc
                        .name()
                        .await
                        .is_ok_and(|n| n.to_lowercase().contains(&wanted))
                    {
                        return Ok((r, Place::Unknown, None));
                    }
                }
                Err(A11yError::NoApp(format!("name {name:?}")))
            }
            Scope::Window { window } => {
                let w = windows
                    .iter()
                    .find(|w| w.id == *window)
                    .ok_or_else(|| A11yError::NoFrame(window.to_string()))?;
                let (frame, _) = self.window_frame(w, windows).await?;
                Ok((frame, Place::Frame(w.clone()), None))
            }
            Scope::Node { node } => {
                let (place, parent) = self.place_of(node, windows).await;
                Ok((node.clone(), place, parent))
            }
        }
    }
}
