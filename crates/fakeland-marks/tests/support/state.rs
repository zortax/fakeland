//! The state of the test compositor.

use std::collections::HashMap;
use std::sync::mpsc::Sender;

use fakeland_protocol::server::fakeland_marks_manager_v1::FakelandMarksManagerV1;
use wayland_server::Resource;
use wayland_server::backend::{ClientData, ClientId, DisconnectReason, ObjectId};
use wayland_server::protocol::wl_pointer::WlPointer;

use super::record::{CommitMarks, Record};

/// The compositor state that every dispatch implementation reads and writes.
pub struct ServerState {
    pub records: Sender<Record>,
    pub managers: Vec<FakelandMarksManagerV1>,
    pub pointers: Vec<WlPointer>,
    /// Pending marks by the object id of the wl_surface that has a marks object.
    pub pending: HashMap<ObjectId, CommitMarks>,
}

impl ServerState {
    pub fn record(&self, record: Record) {
        let _ = self.records.send(record);
    }

    /// Drops the objects that are gone.
    pub fn prune(&mut self) {
        self.managers.retain(Resource::is_alive);
        self.pointers.retain(Resource::is_alive);
    }
}

/// Client data with no content.
pub struct TestClient;

impl ClientData for TestClient {
    fn initialized(&self, _: ClientId) {}
    fn disconnected(&self, _: ClientId, _: DisconnectReason) {}
}
