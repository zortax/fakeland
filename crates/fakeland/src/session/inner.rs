//! The shared core of every session handle: the command channel into the compositor thread, the
//! thread handle and the facts fixed at start. Dropping the last handle shuts the session down.

use std::future::Future;
use std::sync::Mutex;
use std::thread::{JoinHandle, ThreadId};

use fakeland_trace::Environment;
use fakeland_types::SharedClock;
use futures_channel::oneshot;
use smithay::reexports::calloop::channel::Sender;

use super::SessionError;
use crate::config::SessionConfig;
use crate::server::State;

/// A closure that runs on the compositor thread.
pub type Command = Box<dyn FnOnce(&mut State) + Send>;

/// The shared core of a session.
pub struct SessionInner {
    pub(crate) tx: Sender<Command>,
    pub(crate) thread: Mutex<Option<JoinHandle<()>>>,
    pub(crate) thread_id: ThreadId,
    pub(crate) socket_name: String,
    pub(crate) clock: SharedClock,
    pub(crate) environment: Environment,
    pub(crate) dbus_address: Option<String>,
    pub(crate) config: SessionConfig,
}

impl std::fmt::Debug for SessionInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionInner")
            .field("socket_name", &self.socket_name)
            .field("dbus_address", &self.dbus_address)
            .finish()
    }
}

impl SessionInner {
    /// Sends a closure to the compositor thread and returns its reply.
    pub(crate) fn call<T, F>(
        &self,
        f: F,
    ) -> impl Future<Output = Result<T, SessionError>> + use<T, F>
    where
        T: Send + 'static,
        F: FnOnce(&mut State) -> T + Send + 'static,
    {
        self.call_with(move |state, reply| {
            let _ = reply.send(f(state));
        })
    }

    /// Sends a closure that answers through a reply channel, possibly later.
    pub(crate) fn call_with<T, F>(
        &self,
        f: F,
    ) -> impl Future<Output = Result<T, SessionError>> + use<T, F>
    where
        T: Send + 'static,
        F: FnOnce(&mut State, oneshot::Sender<T>) + Send + 'static,
    {
        let (reply, rx) = oneshot::channel();
        let sent = self
            .tx
            .send(Box::new(move |state: &mut State| f(state, reply)));
        async move {
            sent.map_err(|_| SessionError::Closed)?;
            rx.await.map_err(|_| SessionError::Closed)
        }
    }

    /// Stops the compositor thread and waits for it, unless the caller is that thread.
    pub(crate) fn stop(&self) {
        let _ = self.tx.send(Box::new(|state: &mut State| state.shutdown()));
        if std::thread::current().id() == self.thread_id {
            return;
        }
        let handle = self.thread.lock().ok().and_then(|mut h| h.take());
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

impl Drop for SessionInner {
    fn drop(&mut self) {
        self.stop();
    }
}
