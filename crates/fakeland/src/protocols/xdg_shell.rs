//! `xdg_wm_base` version 6. Toplevels and popups go to the window manager, and acked configures
//! land in the trace.

use smithay::delegate_xdg_shell;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::protocol::{wl_output::WlOutput, wl_seat::WlSeat};
use smithay::utils::Serial;
use smithay::wayland::shell::xdg::{
    Configure, PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};

use crate::server::State;

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.protocols.xdg_shell
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        self.toplevel_created(surface);
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        self.popup_created(surface, positioner);
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        self.popup_repositioned(surface, positioner, token);
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        self.toplevel_maximize_request(&surface, true);
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        self.toplevel_maximize_request(&surface, false);
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, output: Option<WlOutput>) {
        self.toplevel_fullscreen_request(&surface, true, output);
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        self.toplevel_fullscreen_request(&surface, false, None);
    }

    fn ack_configure(&mut self, surface: WlSurface, configure: Configure) {
        self.configure_acked(&surface, configure);
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.toplevel_gone(&surface);
    }

    fn popup_destroyed(&mut self, _surface: PopupSurface) {
        self.space_dirty = true;
    }

    fn app_id_changed(&mut self, surface: ToplevelSurface) {
        self.toplevel_identity_changed(&surface);
    }

    fn title_changed(&mut self, surface: ToplevelSurface) {
        self.toplevel_identity_changed(&surface);
    }
}

delegate_xdg_shell!(State);
