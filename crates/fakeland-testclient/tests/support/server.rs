//! The test compositor: it serves one spawned fakeland-testclient over an inherited socket
//! (WAYLAND_SOCKET), pumps its requests, and injects pointer events announced by input_event.

use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::{Child, Command, ExitStatus};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use fakeland_protocol::MANAGER_VERSION;
use fakeland_protocol::server::fakeland_marks_manager_v1::{FakelandMarksManagerV1, InputKind};
use fakeland_protocol::wire::split_u64;
use fakeland_types::{Clock, MonotonicClock, Nanos};
use rustix::io::{FdFlags, fcntl_setfd};
use wayland_protocols::wp::commit_timing::v1::server::wp_commit_timing_manager_v1::WpCommitTimingManagerV1;
use wayland_protocols::wp::fifo::v1::server::wp_fifo_manager_v1::WpFifoManagerV1;
use wayland_protocols::xdg::shell::server::xdg_wm_base::XdgWmBase;
use wayland_server::Display;
use wayland_server::protocol::wl_compositor::WlCompositor;
use wayland_server::protocol::wl_seat::WlSeat;
use wayland_server::protocol::wl_shm::WlShm;

use super::state::{State, TestClient};

/// How long a test waits for the client.
const TIMEOUT: Nanos = Nanos::from_secs(10);

/// Serializes spawns, so no child inherits the socket of another test.
static SPAWN: Mutex<()> = Mutex::new(());

/// A test compositor with one client process.
pub struct TestCompositor {
    display: Display<State>,
    pub state: State,
    child: Child,
    entered: bool,
}

impl TestCompositor {
    /// Starts the compositor and the client with extra arguments. The client logs to `log`.
    /// With `extensions`, the compositor also offers fakeland_marks_v1, wp_fifo_v1 and
    /// wp_commit_timing_v1.
    pub fn start(extensions: bool, log: &Path, args: &[&str]) -> Self {
        let display = Display::<State>::new().expect("display");
        let dh = display.handle();
        dh.create_global::<State, WlCompositor, ()>(4, ());
        dh.create_global::<State, WlShm, ()>(1, ());
        dh.create_global::<State, XdgWmBase, ()>(1, ());
        dh.create_global::<State, WlSeat, ()>(7, ());
        if extensions {
            dh.create_global::<State, FakelandMarksManagerV1, ()>(MANAGER_VERSION, ());
            dh.create_global::<State, WpFifoManagerV1, ()>(1, ());
            dh.create_global::<State, WpCommitTimingManagerV1, ()>(1, ());
        }
        let (server_end, client_end) = UnixStream::pair().expect("socket pair");
        dh.clone()
            .insert_client(server_end, Arc::new(TestClient))
            .expect("client");
        let child = {
            let _guard = SPAWN.lock().unwrap_or_else(|e| e.into_inner());
            fcntl_setfd(&client_end, FdFlags::empty()).expect("clear cloexec");
            let child = Command::new(env!("CARGO_BIN_EXE_fakeland-testclient"))
                .env_clear()
                .env("WAYLAND_SOCKET", client_end.as_raw_fd().to_string())
                .arg("--log-json")
                .arg(log)
                .args(args)
                .spawn()
                .expect("spawn testclient");
            drop(client_end);
            child
        };
        Self {
            display,
            state: State::default(),
            child,
            entered: false,
        }
    }

    /// Serves requests until `done` holds or the timeout passes.
    pub fn pump_until(&mut self, what: &str, mut done: impl FnMut(&State) -> bool) {
        let deadline = MonotonicClock.now() + TIMEOUT;
        while !done(&self.state) {
            assert!(MonotonicClock.now() < deadline, "timeout: {what}");
            self.pump(Duration::from_millis(1));
        }
    }

    /// Serves requests for a while.
    pub fn pump_for(&mut self, d: Nanos) {
        let end = MonotonicClock.now() + d;
        while MonotonicClock.now() < end {
            self.pump(Duration::from_millis(1));
        }
    }

    /// Serves requests until the client exits.
    pub fn wait_exit(&mut self) -> ExitStatus {
        let deadline = MonotonicClock.now() + TIMEOUT;
        loop {
            // A dispatch error means the client is gone, which the exit status reports.
            let _ = self.display.dispatch_clients(&mut self.state);
            let _ = self.display.flush_clients();
            if let Some(status) = self.child.try_wait().expect("wait") {
                return status;
            }
            assert!(MonotonicClock.now() < deadline, "client did not exit");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn pump(&mut self, sleep: Duration) {
        self.display
            .dispatch_clients(&mut self.state)
            .expect("dispatch");
        self.display.flush_clients().expect("flush");
        std::thread::sleep(sleep);
    }

    /// Announces an input id and sends a pointer motion group to the client.
    pub fn pointer_to(&mut self, id: u64, x: f64, y: f64) {
        let (hi, lo) = split_u64(id);
        for m in &self.state.managers {
            m.input_event(hi, lo, InputKind::PointerMotion);
        }
        let surface = self.state.surface.clone().expect("surface");
        let serial = self.state.next_serial();
        for p in &self.state.pointers {
            if self.entered {
                p.motion(serial, x, y);
            } else {
                p.enter(serial, &surface, x, y);
            }
            p.frame();
        }
        self.entered = true;
        self.display.flush_clients().expect("flush");
    }

    /// Sends xdg_toplevel.close and waits for the client to exit.
    pub fn close(&mut self) -> ExitStatus {
        if let Some(top) = &self.state.toplevel {
            top.close();
        }
        self.wait_exit()
    }
}

impl Drop for TestCompositor {
    fn drop(&mut self) {
        // Only the child of this test is killed, by its handle.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
