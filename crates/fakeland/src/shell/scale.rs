//! Preferred scale and transform of surfaces: the fractional scale of `wp_fractional_scale_v1` and
//! the integer buffer scale and transform of `wl_surface` version 6, from the output of the window.

use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::{
    TraversalAction, send_surface_state, with_states, with_surface_tree_downward,
};
use smithay::wayland::fractional_scale::with_fractional_scale;

use crate::outputs::smithay_transform;
use crate::server::State;

impl State {
    /// Returns the output of a surface: the output of its window, or the primary output.
    fn surface_scale_output(
        &self,
        surface: &WlSurface,
    ) -> Option<(f64, fakeland_scenario::Transform)> {
        let output = self
            .window_of_surface(surface)
            .and_then(|w| self.windows.get(w))
            .map(|w| w.output)
            .or_else(|| self.outputs.primary())?;
        let o = self.outputs.get(output)?;
        Some((o.config.scale, o.config.transform))
    }

    /// Sends the preferred scale and transform of one surface.
    pub fn send_surface_scale(&mut self, surface: &WlSurface) {
        if !surface.is_alive() {
            return;
        }
        let Some((scale, transform)) = self.surface_scale_output(surface) else {
            return;
        };
        with_states(surface, |states| {
            with_fractional_scale(states, |fs| fs.set_preferred_scale(scale));
            send_surface_state(
                surface,
                states,
                scale.ceil() as i32,
                smithay_transform(transform),
            );
        });
    }

    /// Sends the preferred scale and transform to every surface of a tree and its popups.
    pub fn send_tree_scale(&mut self, root: &WlSurface) {
        let mut surfaces = Vec::new();
        with_surface_tree_downward(
            root,
            (),
            |_, _, _| TraversalAction::DoChildren(()),
            |s, _, _| surfaces.push(s.clone()),
            |_, _, _| true,
        );
        for (popup, _) in smithay::desktop::PopupManager::popups_for_surface(root) {
            surfaces.push(popup.wl_surface().clone());
        }
        for s in surfaces {
            self.send_surface_scale(&s);
        }
    }
}
