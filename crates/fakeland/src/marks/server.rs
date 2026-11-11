//! The manager objects of every client. Before an injected event reaches a client, every manager
//! of that client gets `input_event` with the input id, so both land in the same flush.

use std::collections::HashMap;

use fakeland_protocol::server::fakeland_marks_manager_v1::{FakelandMarksManagerV1, InputKind};
use fakeland_protocol::wire::split_u64;
use fakeland_types::{ClientId, InputEventId};
use smithay::reexports::wayland_server::Resource;

/// Manager objects by client.
#[derive(Debug, Default)]
pub struct MarksServer {
    managers: HashMap<ClientId, Vec<FakelandMarksManagerV1>>,
}

impl MarksServer {
    /// Adds a bound manager.
    pub fn add(&mut self, client: ClientId, manager: FakelandMarksManagerV1) {
        self.managers.entry(client).or_default().push(manager);
    }

    /// Removes a manager.
    pub fn remove(&mut self, manager: &FakelandMarksManagerV1) {
        self.managers.retain(|_, list| {
            list.retain(|m| m != manager);
            !list.is_empty()
        });
    }

    /// Sends the id of the next injected event to every manager of a client.
    pub fn notify(&self, client: ClientId, id: InputEventId, kind: InputKind) {
        let Some(list) = self.managers.get(&client) else {
            return;
        };
        let (hi, lo) = split_u64(id.0);
        for m in list.iter().filter(|m| m.is_alive()) {
            m.input_event(hi, lo, kind);
        }
    }
}
