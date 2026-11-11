//! The per-iteration work of the event loop and the session lifecycle: client connects and
//! disconnects, the work after each dispatch, and shutdown.

use std::os::unix::net::UnixStream;
use std::sync::Arc;

use fakeland_trace::Event;
use fakeland_types::{ClientId, ProcessId};
use smithay::reexports::rustix::net::sockopt::socket_peercred;

use super::{ClientInfo, ClientState, KILL_GRACE, State};

impl State {
    /// Accepts a client from the listening socket.
    pub fn accept_client(&mut self, stream: UnixStream) {
        let pid = socket_peercred(&stream)
            .ok()
            .map(|c| ProcessId(c.pid.as_raw_nonzero().get() as u32));
        let id = self.ids.client();
        let data = Arc::new(ClientState::new(id, pid, self.clients.disconnected.clone()));
        match self.dh.insert_client(stream, data) {
            Ok(_) => {
                let app = pid.and_then(|p| self.processes.root_of(p));
                self.clients.list.insert(id, ClientInfo { id, pid, app });
                self.emit(Event::ClientConnected { client: id, pid });
            }
            Err(e) => tracing::warn!("cannot add a client: {e}"),
        }
    }

    fn client_gone(&mut self, id: ClientId) {
        if self.clients.list.remove(&id).is_some() {
            self.emit(Event::ClientDisconnected { client: id });
        }
    }

    /// Runs after each dispatch: disconnects, space refresh, observers, waits and the timer.
    pub fn after_dispatch(&mut self) {
        for id in self.clients.take_disconnected() {
            self.client_gone(id);
        }
        if self.space_dirty {
            self.space_dirty = false;
            self.space.refresh();
            self.popups.cleanup();
        }
        self.run_observers();
        self.poll_waiters();
        self.run_observers();
        self.rearm_timer();
    }

    /// Ends the session: children end, waits end, subscribers close and the loop stops. The
    /// socket and the private bus go away when the state drops.
    pub fn shutdown(&mut self) {
        if !self.running {
            return;
        }
        self.running = false;
        self.close_runs();
        self.stop_sampling();
        self.stop_recordings();
        #[cfg(feature = "viewer")]
        self.stop_viewer();
        self.processes.kill_all(KILL_GRACE);
        self.processes.a11y = None;
        self.processes.dbus = None;
        self.cancel_waiters();
        self.bus.close_subscribers();
        self.loop_signal.stop();
        self.loop_signal.wakeup();
    }
}
