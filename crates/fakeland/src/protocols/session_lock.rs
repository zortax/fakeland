//! `ext_session_lock_manager_v1` version 1. Lock requests, lock surfaces and unlock go to the
//! session lock.

use smithay::delegate_session_lock;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::wayland::session_lock::{
    LockSurface, SessionLockHandler, SessionLockManagerState, SessionLocker,
};

use crate::server::State;

impl SessionLockHandler for State {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        &mut self.protocols.session_lock
    }

    fn lock(&mut self, confirmation: SessionLocker) {
        self.lock_requested(confirmation);
    }

    fn unlock(&mut self) {
        self.unlock_requested();
    }

    fn new_surface(&mut self, surface: LockSurface, output: WlOutput) {
        self.lock_surface_created(surface, output);
    }
}

delegate_session_lock!(State);
