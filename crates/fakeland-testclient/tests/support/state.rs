//! The state of the test compositor and what it records.

use std::collections::HashMap;

use fakeland_protocol::server::fakeland_marks_manager_v1::FakelandMarksManagerV1;
use fakeland_types::Timestamp;
use wayland_protocols::xdg::shell::server::{xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel};
use wayland_server::backend::{ClientData, ClientId, DisconnectReason, ObjectId};
use wayland_server::protocol::wl_buffer::WlBuffer;
use wayland_server::protocol::wl_callback::WlCallback;
use wayland_server::protocol::wl_pointer::WlPointer;
use wayland_server::protocol::wl_surface::WlSurface;

/// A span as the compositor decoded it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Span {
    pub name: String,
    pub begin: Timestamp,
    pub end: Timestamp,
}

/// The marks of one commit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitMarks {
    pub frame: Option<u64>,
    pub handled: Vec<u64>,
    pub spans: Vec<Span>,
    pub marks: Vec<String>,
}

/// One surface commit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub buffer: bool,
    pub marks: CommitMarks,
    pub timing: CommitTiming,
}

/// The fifo and commit-timing state of one commit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitTiming {
    pub wait_barrier: bool,
    pub set_barrier: bool,
    pub target: Option<Timestamp>,
}

/// The surface state that waits for a commit.
#[derive(Debug, Default)]
pub struct PendingSurface {
    pub buffer: Option<WlBuffer>,
    pub attached: bool,
    pub callbacks: Vec<WlCallback>,
    pub timing: CommitTiming,
}

/// Everything the test compositor knows.
#[derive(Default)]
pub struct State {
    pub managers: Vec<FakelandMarksManagerV1>,
    pub pointers: Vec<WlPointer>,
    pub surface: Option<WlSurface>,
    pub xdg_surface: Option<XdgSurface>,
    pub toplevel: Option<XdgToplevel>,
    pub configured: bool,
    pub pending: HashMap<ObjectId, PendingSurface>,
    pub marks: HashMap<ObjectId, CommitMarks>,
    pub shown: Option<WlBuffer>,
    pub commits: Vec<Commit>,
    pub client_marks: Vec<String>,
    pub serial: u32,
}

impl State {
    pub fn next_serial(&mut self) -> u32 {
        self.serial += 1;
        self.serial
    }
}

/// Client data with no content.
pub struct TestClient;

impl ClientData for TestClient {
    fn initialized(&self, _: ClientId) {}
    fn disconnected(&self, _: ClientId, _: DisconnectReason) {}
}
