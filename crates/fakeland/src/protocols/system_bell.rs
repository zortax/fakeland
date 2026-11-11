//! `xdg_system_bell_v1`. The session has no sound, so the bell rings silently.

use smithay::delegate_xdg_system_bell;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::xdg_system_bell::XdgSystemBellHandler;

use crate::server::State;

impl XdgSystemBellHandler for State {
    fn ring(&mut self, _surface: Option<WlSurface>) {}
}

delegate_xdg_system_bell!(State);
