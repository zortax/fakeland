//! `ext_session_lock_manager_v1` version 1. Lock requests, lock surfaces and unlock go to the
//! session lock. The lock object dispatch records the sender of each request, so the session lock
//! can tell the holder from other lock objects and route their requests.

use smithay::reexports::wayland_protocols::ext::session_lock::v1::server::ext_session_lock_manager_v1::ExtSessionLockManagerV1;
use smithay::reexports::wayland_protocols::ext::session_lock::v1::server::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1;
use smithay::reexports::wayland_protocols::ext::session_lock::v1::server::ext_session_lock_v1::{
    ExtSessionLockV1, Request,
};
use smithay::reexports::wayland_server::backend::ClientId;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, delegate_dispatch, delegate_global_dispatch,
};
use smithay::wayland::session_lock::{
    ExtLockSurfaceUserData, LockSurface, SessionLockHandler, SessionLockManagerGlobalData,
    SessionLockManagerState, SessionLockState, SessionLocker,
};

use crate::server::State;

impl SessionLockHandler for State {
    fn lock_state(&mut self) -> &mut SessionLockManagerState {
        self.routed_lock_state()
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

impl Dispatch<ExtSessionLockV1, SessionLockState> for State {
    fn request(
        state: &mut Self,
        client: &Client,
        lock: &ExtSessionLockV1,
        request: Request,
        data: &SessionLockState,
        dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        state.lock.sender = Some(lock.clone());
        if let Request::GetLockSurface { output, .. } = &request {
            state.prepare_scratch_lock_surface(output);
        }
        <SessionLockManagerState as Dispatch<ExtSessionLockV1, SessionLockState, Self>>::request(
            state, client, lock, request, data, dh, data_init,
        );
        state.lock.sender = None;
    }

    fn destroyed(
        state: &mut Self,
        client: ClientId,
        lock: &ExtSessionLockV1,
        data: &SessionLockState,
    ) {
        <SessionLockManagerState as Dispatch<ExtSessionLockV1, SessionLockState, Self>>::destroyed(
            state, client, lock, data,
        );
    }
}

delegate_global_dispatch!(State: [ExtSessionLockManagerV1: SessionLockManagerGlobalData] => SessionLockManagerState);
delegate_dispatch!(State: [ExtSessionLockManagerV1: ()] => SessionLockManagerState);
delegate_dispatch!(State: [ExtSessionLockSurfaceV1: ExtLockSurfaceUserData] => SessionLockManagerState);
