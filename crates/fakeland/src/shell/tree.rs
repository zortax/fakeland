//! The surfaces of a window tree with their global origins: the toplevel, layer or lock surface,
//! its subsurfaces, and popups with their subsurfaces.

use fakeland_types::{SurfaceId, WindowId};
use smithay::desktop::PopupManager;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::{
    SubsurfaceCachedState, TraversalAction, with_surface_tree_downward,
};

use crate::server::State;

/// One surface of a window tree.
#[derive(Clone, Debug)]
pub struct TreeSurface {
    /// The surface.
    pub surface: WlSurface,
    /// The session id.
    pub id: SurfaceId,
    /// The origin of the surface in the global logical space.
    pub origin: Point<i32, Logical>,
}

impl State {
    /// Returns every surface of a mapped window tree, root first, with global origins.
    pub fn window_tree(&self, id: WindowId) -> Vec<TreeSurface> {
        let Some(rec) = self.windows.get(id) else {
            return self.shell_surface_tree(id);
        };
        let Some(location) = self.space.element_location(&rec.window) else {
            return Vec::new();
        };
        let render = location - rec.window.geometry().loc;
        let root = rec.toplevel.wl_surface().clone();
        let mut out = Vec::new();
        self.collect_subtree(&root, render, &mut out);
        for (popup, at) in PopupManager::popups_for_surface(&root) {
            let origin = render + rec.window.geometry().loc + at - popup.geometry().loc;
            self.collect_subtree(popup.wl_surface(), origin, &mut out);
        }
        out
    }

    /// Returns every surface of a mapped layer or lock surface tree, root first, with global
    /// origins.
    fn shell_surface_tree(&self, id: WindowId) -> Vec<TreeSurface> {
        let Some((root, origin)) = self.shell_root(id) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        self.collect_subtree(&root, origin, &mut out);
        for (popup, at) in PopupManager::popups_for_surface(&root) {
            self.collect_subtree(
                popup.wl_surface(),
                origin + at - popup.geometry().loc,
                &mut out,
            );
        }
        out
    }

    /// Returns the root surface and global origin of a mapped layer or lock surface.
    pub fn shell_root(&self, id: WindowId) -> Option<(WlSurface, Point<i32, Logical>)> {
        if let Some(rec) = self.layers.get(id).filter(|l| l.mapped_at.is_some()) {
            return Some((rec.layer.wl_surface().clone(), self.layer_origin(id)?));
        }
        let rec = self.lock.get(id).filter(|l| l.mapped_at.is_some())?;
        Some((rec.surface.wl_surface().clone(), self.lock_origin(id)?))
    }

    fn collect_subtree(
        &self,
        root: &WlSurface,
        origin: Point<i32, Logical>,
        out: &mut Vec<TreeSurface>,
    ) {
        let mut found = Vec::new();
        with_surface_tree_downward(
            root,
            origin,
            |_, states, loc| {
                let mut loc = *loc;
                if states.role == Some("subsurface") {
                    loc += states
                        .cached_state
                        .get::<SubsurfaceCachedState>()
                        .current()
                        .location;
                }
                TraversalAction::DoChildren(loc)
            },
            |surface, states, loc| {
                let mut loc = *loc;
                if states.role == Some("subsurface") {
                    loc += states
                        .cached_state
                        .get::<SubsurfaceCachedState>()
                        .current()
                        .location;
                }
                found.push((surface.clone(), loc));
            },
            |_, _, _| true,
        );
        out.extend(found.into_iter().filter_map(|(surface, origin)| {
            let id = self.surfaces.id_of(&surface)?;
            Some(TreeSurface {
                surface,
                id,
                origin,
            })
        }));
    }
}
