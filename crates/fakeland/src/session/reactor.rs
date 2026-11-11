//! Reactor calls of the session: install a reactor on the compositor thread and get its handle.

use std::sync::{Arc, Mutex};

use futures_channel::mpsc;

use super::{Session, SessionError};
use crate::reactor::{Reactor, ReactorHandle};
use crate::util::block_on;

impl Session {
    /// Installs a reactor. It sees every trace record from now on, until it finishes or the
    /// handle removes it.
    pub async fn install_reactor<R: Reactor>(
        &self,
        reactor: R,
    ) -> Result<ReactorHandle<R::Value>, SessionError> {
        let (tx, rx) = mpsc::unbounded();
        let id = self
            .inner
            .call(move |state| state.install_reactor(reactor, tx))
            .await?;
        Ok(ReactorHandle {
            id,
            inner: Arc::downgrade(&self.inner),
            rx: Mutex::new(rx),
        })
    }

    /// Installs a reactor on the calling thread.
    pub fn blocking_install_reactor<R: Reactor>(
        &self,
        reactor: R,
    ) -> Result<ReactorHandle<R::Value>, SessionError> {
        block_on(self.install_reactor(reactor))
    }
}
