//! Per-client state: the compositor transaction queue, the client id and the peer pid. A
//! disconnect lands in a shared queue that the event loop drains.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use fakeland_types::{ClientId, ProcessId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smithay::reexports::wayland_server::Client;
use smithay::reexports::wayland_server::backend::{
    ClientData, ClientId as WlClientId, DisconnectReason,
};
use smithay::wayland::compositor::CompositorClientState;

/// The user data of every client.
#[derive(Debug)]
pub struct ClientState {
    /// The smithay compositor state of the client.
    pub compositor: CompositorClientState,
    /// The session id of the client.
    pub id: ClientId,
    /// The peer pid from the socket credentials.
    pub pid: Option<ProcessId>,
    disconnected: Arc<Mutex<Vec<ClientId>>>,
}

impl ClientState {
    /// Builds the state of a new client.
    pub fn new(
        id: ClientId,
        pid: Option<ProcessId>,
        disconnected: Arc<Mutex<Vec<ClientId>>>,
    ) -> Self {
        Self {
            compositor: CompositorClientState::default(),
            id,
            pid,
            disconnected,
        }
    }
}

impl ClientData for ClientState {
    fn disconnected(&self, _client_id: WlClientId, _reason: DisconnectReason) {
        if let Ok(mut list) = self.disconnected.lock() {
            list.push(self.id);
        }
    }
}

/// One connected client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ClientInfo {
    /// The client id.
    pub id: ClientId,
    /// The peer pid.
    pub pid: Option<ProcessId>,
    /// The launched process that the client descends from.
    pub app: Option<ProcessId>,
}

/// Every connected client.
#[derive(Debug, Default)]
pub struct Clients {
    /// Clients by id.
    pub list: HashMap<ClientId, ClientInfo>,
    /// Disconnects reported by clients, drained after each dispatch.
    pub disconnected: Arc<Mutex<Vec<ClientId>>>,
}

impl Clients {
    /// Returns the session id of a wayland client.
    pub fn id_of(client: &Client) -> Option<ClientId> {
        client.get_data::<ClientState>().map(|c| c.id)
    }

    /// Takes the disconnects reported since the last call.
    pub fn take_disconnected(&self) -> Vec<ClientId> {
        self.disconnected
            .lock()
            .map(|mut l| std::mem::take(&mut *l))
            .unwrap_or_default()
    }
}
