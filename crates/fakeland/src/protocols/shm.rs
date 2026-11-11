//! `wl_shm` and the buffer destruction hook of every buffer kind.

use smithay::delegate_shm;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::shm::{ShmHandler, ShmState};

use crate::server::State;

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, buffer: &WlBuffer) {
        self.frames.buffer_ids.forget(buffer);
    }
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.protocols.shm
    }
}

delegate_shm!(State);
