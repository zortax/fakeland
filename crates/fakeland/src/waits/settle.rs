//! Settle detection: a window is settled when no surface of its tree committed damage for a quiet
//! span after it mapped.

use fakeland_types::{Timestamp, WindowId};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::{TraversalAction, with_surface_tree_downward};

use crate::server::State;

impl State {
    /// Returns the time since which a mapped window has been quiet: its last damage commit, or
    /// its map time.
    pub fn quiet_since(&self, id: WindowId) -> Option<Timestamp> {
        let (root, mapped) = self.settle_root(id)?;
        let mut surfaces = Vec::new();
        with_surface_tree_downward(
            &root,
            (),
            |_, _, _| TraversalAction::DoChildren(()),
            |s, _, _| surfaces.push(s.clone()),
            |_, _, _| true,
        );
        for (popup, _) in smithay::desktop::PopupManager::popups_for_surface(&root) {
            surfaces.push(popup.wl_surface().clone());
        }
        let last = surfaces
            .iter()
            .filter_map(|s| self.surfaces.id_of(s))
            .filter_map(|s| self.surfaces.get(s)?.last_damage)
            .max();
        Some(last.map_or(mapped, |l| l.max(mapped)))
    }

    /// Returns the root surface and map time of a mapped toplevel, layer surface or lock surface.
    fn settle_root(&self, id: WindowId) -> Option<(WlSurface, Timestamp)> {
        if let Some(rec) = self.windows.get(id) {
            return Some((rec.toplevel.wl_surface().clone(), rec.mapped_at?));
        }
        if let Some(rec) = self.layers.get(id) {
            return Some((rec.layer.wl_surface().clone(), rec.mapped_at?));
        }
        let rec = self.lock.get(id)?;
        Some((rec.surface.wl_surface().clone(), rec.mapped_at?))
    }
}
