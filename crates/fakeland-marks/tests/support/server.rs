//! The test compositor thread. It serves one client over a socket pair, runs commands from the
//! test and reports records.

use std::collections::HashMap;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use fakeland_protocol::MANAGER_VERSION;
use fakeland_protocol::server::fakeland_marks_manager_v1::{FakelandMarksManagerV1, InputKind};
use fakeland_protocol::wire::split_u64;
use fakeland_types::InputEventId;
use wayland_server::Display;
use wayland_server::protocol::wl_compositor::WlCompositor;
use wayland_server::protocol::wl_seat::WlSeat;

use super::record::Record;
use super::state::{ServerState, TestClient};

const POLL: Duration = Duration::from_millis(1);
const TIMEOUT: Duration = Duration::from_secs(10);

enum Command {
    Input {
        id: InputEventId,
        kind: InputKind,
        done: Sender<()>,
    },
    Stop,
}

/// A running test compositor with one connected client.
pub struct TestServer {
    commands: Sender<Command>,
    records: Receiver<Record>,
    thread: Option<JoinHandle<()>>,
}

impl TestServer {
    /// Starts a compositor and returns it with the client end of its socket. The compositor
    /// advertises the marks global when `marks` is true.
    pub fn start(marks: bool) -> (Self, UnixStream) {
        let (server_end, client_end) = UnixStream::pair().expect("socket pair");
        let (commands, command_rx) = mpsc::channel();
        let (record_tx, records) = mpsc::channel();
        let thread = thread::spawn(move || serve(server_end, marks, command_rx, record_tx));
        let server = Self {
            commands,
            records,
            thread: Some(thread),
        };
        (server, client_end)
    }

    /// Sends an input_event and a wl_pointer motion group to the client, and waits until the
    /// compositor flushed both.
    pub fn send_input(&self, id: InputEventId, kind: InputKind) {
        let (done, done_rx) = mpsc::channel();
        self.commands
            .send(Command::Input { id, kind, done })
            .expect("server alive");
        done_rx.recv_timeout(TIMEOUT).expect("input flushed");
    }

    /// Returns the next record.
    pub fn next_record(&self) -> Record {
        self.records.recv_timeout(TIMEOUT).expect("record")
    }

    /// Returns true when no record waits.
    pub fn no_records(&self) -> bool {
        matches!(
            self.records.recv_timeout(Duration::from_millis(20)),
            Err(RecvTimeoutError::Timeout)
        )
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Stop);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("server thread");
        }
    }
}

fn serve(stream: UnixStream, marks: bool, commands: Receiver<Command>, records: Sender<Record>) {
    let mut display = Display::<ServerState>::new().expect("display");
    let dh = display.handle();
    dh.create_global::<ServerState, WlCompositor, ()>(4, ());
    dh.create_global::<ServerState, WlSeat, ()>(5, ());
    if marks {
        dh.create_global::<ServerState, FakelandMarksManagerV1, ()>(MANAGER_VERSION, ());
    }
    display
        .handle()
        .insert_client(stream, Arc::new(TestClient))
        .expect("client");
    let mut state = ServerState {
        records,
        managers: Vec::new(),
        pointers: Vec::new(),
        pending: HashMap::new(),
    };
    loop {
        display.dispatch_clients(&mut state).expect("dispatch");
        state.prune();
        match commands.try_recv() {
            Ok(Command::Input { id, kind, done }) => {
                send_input(&state, id, kind);
                display.flush_clients().expect("flush");
                let _ = done.send(());
            }
            Ok(Command::Stop) | Err(mpsc::TryRecvError::Disconnected) => return,
            Err(mpsc::TryRecvError::Empty) => {}
        }
        display.flush_clients().expect("flush");
        thread::sleep(POLL);
    }
}

fn send_input(state: &ServerState, id: InputEventId, kind: InputKind) {
    let (hi, lo) = split_u64(id.0);
    for manager in &state.managers {
        manager.input_event(hi, lo, kind);
    }
    for pointer in &state.pointers {
        pointer.motion(id.0 as u32, 10.0, 20.0);
        pointer.frame();
    }
}
