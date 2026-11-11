//! Popups: tracking, and positioner constraint against the output of the parent window.

use smithay::desktop::{PopupKind, find_popup_root_surface, get_popup_toplevel_coords};
use smithay::utils::Rectangle;
use smithay::wayland::shell::xdg::{PopupSurface, PositionerState};

use crate::server::State;

impl State {
    /// Registers a new popup with its constrained geometry.
    pub fn popup_created(&mut self, popup: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&popup);
        let parent = popup.get_parent_surface();
        self.surface_role(popup.wl_surface(), "xdg_popup", parent.as_ref());
        if let Err(e) = self.popups.track_popup(PopupKind::Xdg(popup)) {
            tracing::debug!("popup not tracked: {e}");
        }
    }

    /// Handles `xdg_popup.reposition`.
    pub fn popup_repositioned(
        &mut self,
        popup: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        popup.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        self.unconstrain_popup(&popup);
        popup.send_repositioned(token);
    }

    /// Keeps a popup inside the output of its window.
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let kind = PopupKind::Xdg(popup.clone());
        let Ok(root) = find_popup_root_surface(&kind) else {
            return;
        };
        let Some(rec) = self
            .window_of_surface(&root)
            .and_then(|w| self.windows.get(w))
        else {
            return;
        };
        let Some(out) = self.outputs.get(rec.output) else {
            return;
        };
        let out_rect = out.logical_rect();
        let window_loc = self
            .space
            .element_geometry(&rec.window)
            .map(|g| g.loc)
            .unwrap_or_else(|| (rec.floating.x, rec.floating.y).into());
        let mut target = Rectangle::new(
            (out_rect.x, out_rect.y).into(),
            (out_rect.w, out_rect.h).into(),
        );
        target.loc -= get_popup_toplevel_coords(&kind);
        target.loc -= window_loc;
        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}
