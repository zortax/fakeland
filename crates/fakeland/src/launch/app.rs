//! A launched app: its pid, captured output, exit status and windows. The handle keeps the session
//! alive only weakly.

use std::sync::{Arc, Mutex, Weak};

use fakeland_types::{Nanos, ProcessId};
use futures_channel::oneshot;
use smithay::reexports::rustix::io::Errno;
use smithay::reexports::rustix::process::{Pid, Signal, kill_process_group};

use super::{ExitInfo, RingBuffer, proctree};
use crate::session::{Session, SessionError, SessionInner};
use crate::shell::WindowInfo;
use crate::util::block_on;

/// The state of a child that its threads and handles share.
#[derive(Debug)]
pub struct AppShared {
    /// The pid, which is also the process group id.
    pub pid: ProcessId,
    /// The command line.
    pub command: String,
    /// The tail of stdout.
    pub stdout: Mutex<RingBuffer>,
    /// The tail of stderr.
    pub stderr: Mutex<RingBuffer>,
    exit: Mutex<Option<ExitInfo>>,
    waiters: Mutex<Vec<oneshot::Sender<ExitInfo>>>,
}

impl AppShared {
    /// Builds the shared state of a new child.
    pub fn new(pid: ProcessId, command: String, log_capacity: usize) -> Self {
        Self {
            pid,
            command,
            stdout: Mutex::new(RingBuffer::new(log_capacity)),
            stderr: Mutex::new(RingBuffer::new(log_capacity)),
            exit: Mutex::new(None),
            waiters: Mutex::new(Vec::new()),
        }
    }

    /// Records the exit and wakes every waiter.
    pub fn set_exit(&self, info: ExitInfo) {
        if let Ok(mut e) = self.exit.lock() {
            *e = Some(info);
        }
        let waiters = self
            .waiters
            .lock()
            .map(|mut w| std::mem::take(&mut *w))
            .unwrap_or_default();
        for w in waiters {
            let _ = w.send(info);
        }
    }

    /// Returns the exit status once the child ended.
    pub fn exit(&self) -> Option<ExitInfo> {
        self.exit.lock().ok().and_then(|e| *e)
    }

    /// Sends a signal to the process group. The group outlives its leader while another member
    /// runs, so the signal also goes out after the leader exits. A group without members is no
    /// error.
    pub fn signal(&self, signal: Signal) -> std::io::Result<()> {
        if !self.group_alive() {
            return Ok(());
        }
        let pid =
            Pid::from_raw(self.pid.0 as i32).ok_or_else(|| std::io::Error::other("invalid pid"))?;
        match kill_process_group(pid, signal) {
            Ok(()) | Err(Errno::SRCH) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Returns true while the process group has a member. After the leader exits, only a
    /// member in the session of this process counts, so a recycled group id never does.
    pub fn group_alive(&self) -> bool {
        if self.exit().is_none() {
            return true;
        }
        proctree::own_session()
            .is_some_and(|sid| proctree::group_has_member(proctree::all_stats(), self.pid.0, sid))
    }

    fn wait_channel(&self) -> oneshot::Receiver<ExitInfo> {
        let (tx, rx) = oneshot::channel();
        let mut waiters = self.waiters.lock().unwrap_or_else(|e| e.into_inner());
        match self.exit() {
            Some(info) => {
                let _ = tx.send(info);
            }
            None => waiters.push(tx),
        }
        rx
    }
}

/// A launched app.
#[derive(Debug, Clone)]
pub struct App {
    shared: Arc<AppShared>,
    session: Weak<SessionInner>,
}

impl App {
    pub(crate) fn new(shared: Arc<AppShared>, session: Weak<SessionInner>) -> Self {
        Self { shared, session }
    }

    /// Returns the pid. The pid is also the process group of the app.
    pub fn pid(&self) -> ProcessId {
        self.shared.pid
    }

    /// Returns the command line.
    pub fn command(&self) -> &str {
        &self.shared.command
    }

    /// Returns the kept tail of stdout.
    pub fn stdout(&self) -> String {
        self.shared
            .stdout
            .lock()
            .map(|r| r.text())
            .unwrap_or_default()
    }

    /// Returns the kept tail of stderr.
    pub fn stderr(&self) -> String {
        self.shared
            .stderr
            .lock()
            .map(|r| r.text())
            .unwrap_or_default()
    }

    /// Returns the exit status once the app ended.
    pub fn try_wait(&self) -> Option<ExitInfo> {
        self.shared.exit()
    }

    /// Waits for the app to end.
    pub async fn wait(&self) -> ExitInfo {
        let rx = self.shared.wait_channel();
        rx.await.unwrap_or(ExitInfo {
            code: None,
            signal: None,
        })
    }

    /// Waits for the app to end on the calling thread.
    pub fn blocking_wait(&self) -> ExitInfo {
        block_on(self.wait())
    }

    /// Sends SIGTERM to the process group, also after the app itself exited.
    pub fn terminate(&self) -> std::io::Result<()> {
        self.shared.signal(Signal::TERM)
    }

    /// Sends SIGKILL to the process group, also after the app itself exited.
    pub fn kill(&self) -> std::io::Result<()> {
        self.shared.signal(Signal::KILL)
    }

    fn session(&self) -> Result<Session, SessionError> {
        self.session
            .upgrade()
            .map(Session::from_inner)
            .ok_or(SessionError::Closed)
    }

    /// Returns the windows of the app and of its descendants.
    pub async fn windows(&self) -> Result<Vec<WindowInfo>, SessionError> {
        self.session()?.app_windows(self.pid()).await
    }

    /// Returns the windows of the app on the calling thread.
    pub fn blocking_windows(&self) -> Result<Vec<WindowInfo>, SessionError> {
        block_on(self.windows())
    }

    /// Waits for the first mapped window of the app.
    pub async fn wait_window(&self, timeout: Nanos) -> Result<WindowInfo, SessionError> {
        self.session()?.wait_app_window(self.pid(), timeout).await
    }

    /// Waits for the first mapped window of the app on the calling thread.
    pub fn blocking_wait_window(&self, timeout: Nanos) -> Result<WindowInfo, SessionError> {
        block_on(self.wait_window(timeout))
    }
}
