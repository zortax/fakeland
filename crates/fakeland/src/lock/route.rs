//! The lock manager state of each lock request. The manager state lists the output of every lock
//! surface and refuses a second lock surface on a listed output. The holder's requests use the
//! session's manager state. Requests of other lock objects, such as a refused lock or the lock of
//! a forced unlock, use a scratch state, so their lock surfaces never collide with the holder's.
//! Each fresh manager state adds one global that no client sees.

use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::wayland::session_lock::SessionLockManagerState;

use crate::server::State;

impl State {
    /// Returns the lock manager state of the request in dispatch.
    pub fn routed_lock_state(&mut self) -> &mut SessionLockManagerState {
        if self.lock.sender.is_none() || self.lock.holder_sent() {
            return &mut self.protocols.session_lock;
        }
        let dh = self.dh.clone();
        self.lock
            .scratch
            .get_or_insert_with(|| SessionLockManagerState::new::<State, _>(&dh, |_| false))
    }

    /// Prepares the scratch state for a lock surface of a lock object other than the holder. A
    /// fresh scratch state replaces one that lists the output already.
    pub fn prepare_scratch_lock_surface(&mut self, output: &WlOutput) {
        if self.lock.holder_sent() {
            return;
        }
        if self.lock.scratch_outputs.contains(output) {
            self.lock.scratch = None;
            self.lock.scratch_outputs.clear();
        }
        self.lock.scratch_outputs.push(output.clone());
    }

    /// Replaces the session's lock manager state. The fresh state lists no output, so it releases
    /// the outputs of a lock that ends without an unlock request.
    pub fn reset_locked_outputs(&mut self) {
        self.protocols.session_lock = SessionLockManagerState::new::<State, _>(&self.dh, |_| false);
    }
}
