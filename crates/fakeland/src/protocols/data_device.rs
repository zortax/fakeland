//! `wl_data_device_manager` and the shared selection handler. Client selections land in the
//! trace. A selection the compositor sets carries its bytes as user data.

use std::io::Write;
use std::os::fd::OwnedFd;
use std::sync::Arc;

use fakeland_trace::Event;
use smithay::delegate_data_device;
use smithay::input::Seat;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::selection::{SelectionHandler, SelectionSource, SelectionTarget};

use crate::server::State;

impl SelectionHandler for State {
    type SelectionUserData = Arc<[u8]>;

    fn new_selection(
        &mut self,
        ty: SelectionTarget,
        source: Option<SelectionSource>,
        _seat: Seat<Self>,
    ) {
        if ty == SelectionTarget::Clipboard && source.is_some() {
            self.selection = None;
        }
        let mime = source.map(|s| s.mime_types()).unwrap_or_default();
        self.emit(Event::Clipboard {
            mime,
            primary: ty == SelectionTarget::Primary,
        });
    }

    fn send_selection(
        &mut self,
        _ty: SelectionTarget,
        _mime_type: String,
        fd: OwnedFd,
        _seat: Seat<Self>,
        user_data: &Self::SelectionUserData,
    ) {
        let bytes = user_data.clone();
        let spawned = std::thread::Builder::new()
            .name("selection-write".into())
            .spawn(move || {
                let mut file = std::fs::File::from(fd);
                if let Err(e) = file.write_all(&bytes) {
                    tracing::debug!("selection write failed: {e}");
                }
            });
        if let Err(e) = spawned {
            tracing::warn!("cannot start the selection writer: {e}");
        }
    }
}

impl ClientDndGrabHandler for State {}

impl ServerDndGrabHandler for State {}

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.protocols.data_device
    }
}

delegate_data_device!(State);
