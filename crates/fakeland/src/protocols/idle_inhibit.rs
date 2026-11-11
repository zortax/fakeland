//! `zwp_idle_inhibit_manager_v1`. The session never idles, so inhibitors only exist.

use smithay::delegate_idle_inhibit;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::idle_inhibit::IdleInhibitHandler;

use crate::server::State;

impl IdleInhibitHandler for State {
    fn inhibit(&mut self, _surface: WlSurface) {}

    fn uninhibit(&mut self, _surface: WlSurface) {}
}

delegate_idle_inhibit!(State);
