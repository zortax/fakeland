//! `zwp_input_method_manager_v2`. Input method popups join the popup manager and sit relative to
//! the window of their parent surface.

use smithay::delegate_input_method_manager;
use smithay::desktop::{PopupKind, PopupManager};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Rectangle};
use smithay::wayland::input_method::{InputMethodHandler, PopupSurface};
use smithay::wayland::seat::WaylandFocus;

use crate::server::State;

impl InputMethodHandler for State {
    fn new_popup(&mut self, surface: PopupSurface) {
        if let Err(e) = self.popups.track_popup(PopupKind::from(surface)) {
            tracing::debug!("input method popup not tracked: {e}");
        }
    }

    fn dismiss_popup(&mut self, surface: PopupSurface) {
        if let Some(parent) = surface.get_parent().map(|p| p.surface.clone()) {
            let _ = PopupManager::dismiss_popup(&parent, &PopupKind::from(surface));
        }
    }

    fn popup_repositioned(&mut self, _surface: PopupSurface) {}

    fn parent_geometry(&self, parent: &WlSurface) -> Rectangle<i32, Logical> {
        self.space
            .elements()
            .find(|w| w.wl_surface().as_deref() == Some(parent))
            .map(|w| w.geometry())
            .unwrap_or_default()
    }
}

delegate_input_method_manager!(State);
