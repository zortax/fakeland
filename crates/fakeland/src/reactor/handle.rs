//! The caller side of a reactor: values it emits and its removal.

use std::sync::{Mutex, Weak};

use futures_channel::mpsc::UnboundedReceiver;
use futures_util::StreamExt;

use crate::session::{SessionError, SessionInner};
use crate::util::block_on;

/// The id of an installed reactor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ReactorId(pub u64);

/// The handle of an installed reactor. Values arrive in emit order; after the reactor finishes
/// or is removed, the remaining values drain and `recv` returns `None`.
pub struct ReactorHandle<V> {
    pub(crate) id: ReactorId,
    pub(crate) inner: Weak<SessionInner>,
    pub(crate) rx: Mutex<UnboundedReceiver<V>>,
}

impl<V> std::fmt::Debug for ReactorHandle<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReactorHandle")
            .field("id", &self.id)
            .finish()
    }
}

impl<V: Send + 'static> ReactorHandle<V> {
    /// Returns the reactor id.
    pub fn id(&self) -> ReactorId {
        self.id
    }

    /// Returns a value when one is queued.
    pub fn try_recv(&self) -> Option<V> {
        let mut rx = self.rx.lock().unwrap_or_else(|e| e.into_inner());
        rx.try_recv().ok()
    }

    /// Returns every queued value.
    pub fn drain(&self) -> Vec<V> {
        std::iter::from_fn(|| self.try_recv()).collect()
    }

    /// Waits for the next value. Returns `None` after the reactor ended and every value
    /// drained.
    pub async fn recv(&self) -> Option<V> {
        std::future::poll_fn(|cx| {
            let mut rx = self.rx.lock().unwrap_or_else(|e| e.into_inner());
            rx.poll_next_unpin(cx)
        })
        .await
    }

    /// Waits for the next value on the calling thread.
    pub fn blocking_recv(&self) -> Option<V> {
        block_on(self.recv())
    }

    /// Removes the reactor.
    pub async fn remove(&self) -> Result<(), SessionError> {
        let inner = self.inner.upgrade().ok_or(SessionError::Closed)?;
        let id = self.id;
        inner.call(move |state| state.remove_reactor(id)).await
    }

    /// Removes the reactor on the calling thread.
    pub fn blocking_remove(&self) -> Result<(), SessionError> {
        block_on(self.remove())
    }
}
