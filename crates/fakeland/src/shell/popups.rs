//! Popups: tracking, and positioner constraint against the output of the parent window or layer
//! surface.

use fakeland_types::OutputId;
use smithay::desktop::{PopupKind, find_popup_root_surface, get_popup_toplevel_coords};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Rectangle};
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

    /// Keeps a popup inside the output of its window or layer surface.
    pub fn unconstrain_popup(&self, popup: &PopupSurface) {
        let kind = PopupKind::Xdg(popup.clone());
        let Ok(root) = find_popup_root_surface(&kind) else {
            return;
        };
        let Some((output, root_loc)) = self.popup_root_place(&root) else {
            return;
        };
        let Some(out) = self.outputs.get(output) else {
            return;
        };
        let out_rect = out.logical_rect();
        let mut target = Rectangle::new(
            (out_rect.x, out_rect.y).into(),
            (out_rect.w, out_rect.h).into(),
        );
        target.loc -= get_popup_toplevel_coords(&kind);
        target.loc -= root_loc;
        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }

    /// Returns the output of the root of a popup tree and the global origin that popup positions
    /// count from: the window geometry of a toplevel, or the surface origin of a layer surface.
    fn popup_root_place(&self, root: &WlSurface) -> Option<(OutputId, Point<i32, Logical>)> {
        let id = self.window_of_surface(root)?;
        if let Some(rec) = self.windows.get(id) {
            let loc = self
                .space
                .element_geometry(&rec.window)
                .map(|g| g.loc)
                .unwrap_or_else(|| (rec.floating.x, rec.floating.y).into());
            return Some((rec.output, loc));
        }
        let rec = self.layers.get(id)?;
        Some((rec.output, self.layer_origin(id)?))
    }
}
