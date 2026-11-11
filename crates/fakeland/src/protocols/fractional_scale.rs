//! `wp_fractional_scale_manager_v1`. A new fractional scale object gets the scale of the surface
//! output at once.

use smithay::delegate_fractional_scale;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::fractional_scale::FractionalScaleHandler;

use crate::server::State;

impl FractionalScaleHandler for State {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        self.send_surface_scale(&surface);
    }
}

delegate_fractional_scale!(State);
