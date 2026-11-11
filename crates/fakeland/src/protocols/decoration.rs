//! `zxdg_decoration_manager_v1`. Every toplevel gets the decoration mode of the session config.

use smithay::delegate_xdg_decoration;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode;
use smithay::wayland::shell::xdg::ToplevelSurface;
use smithay::wayland::shell::xdg::decoration::XdgDecorationHandler;

use crate::config::Decorations;
use crate::server::State;

impl State {
    fn decoration_mode(&self) -> Mode {
        match self.config.decorations {
            Decorations::ClientSide => Mode::ClientSide,
            Decorations::ServerSide => Mode::ServerSide,
        }
    }

    fn apply_decoration(&mut self, toplevel: &ToplevelSurface) {
        let mode = self.decoration_mode();
        toplevel.with_pending_state(|s| s.decoration_mode = Some(mode));
        if toplevel.is_initial_configure_sent() {
            self.send_toplevel_configure(toplevel);
        }
    }
}

impl XdgDecorationHandler for State {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        self.apply_decoration(&toplevel);
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: Mode) {
        self.apply_decoration(&toplevel);
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        self.apply_decoration(&toplevel);
    }
}

delegate_xdg_decoration!(State);
