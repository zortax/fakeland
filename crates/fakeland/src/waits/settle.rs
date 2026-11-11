//! Settle detection: a window is settled when no surface of its tree committed damage for a quiet
//! span after it mapped.

use fakeland_types::{Timestamp, WindowId};
use smithay::wayland::compositor::{TraversalAction, with_surface_tree_downward};

use crate::server::State;

impl State {
    /// Returns the time since which a mapped window has been quiet: its last damage commit, or
    /// its map time.
    pub fn quiet_since(&self, id: WindowId) -> Option<Timestamp> {
        let rec = self.windows.get(id)?;
        let mapped = rec.mapped_at?;
        let mut surfaces = Vec::new();
        with_surface_tree_downward(
            rec.toplevel.wl_surface(),
            (),
            |_, _, _| TraversalAction::DoChildren(()),
            |s, _, _| surfaces.push(s.clone()),
            |_, _, _| true,
        );
        for (popup, _) in
            smithay::desktop::PopupManager::popups_for_surface(rec.toplevel.wl_surface())
        {
            surfaces.push(popup.wl_surface().clone());
        }
        let last = surfaces
            .iter()
            .filter_map(|s| self.surfaces.id_of(s))
            .filter_map(|s| self.surfaces.get(s)?.last_damage)
            .max();
        Some(last.map_or(mapped, |l| l.max(mapped)))
    }
}
