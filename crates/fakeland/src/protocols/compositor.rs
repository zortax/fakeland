//! `wl_compositor` version 6 and `wl_subcompositor`. Surface creation, commits and destruction
//! go to the frame pipeline and the shell.

use smithay::delegate_compositor;
use smithay::reexports::wayland_server::Client;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::{CompositorClientState, CompositorHandler, CompositorState};

use crate::server::{ClientState, State};

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.protocols.compositor
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ClientState>()
            .expect("every client carries ClientState")
            .compositor
    }

    fn new_surface(&mut self, surface: &WlSurface) {
        self.surface_created(surface);
    }

    fn new_subsurface(&mut self, surface: &WlSurface, parent: &WlSurface) {
        self.surface_role(surface, "subsurface", Some(parent));
    }

    fn commit(&mut self, surface: &WlSurface) {
        self.surface_applied(surface);
    }

    fn destroyed(&mut self, surface: &WlSurface) {
        self.surface_destroyed(surface);
    }
}

delegate_compositor!(State);
