//! The session handle: socket and environment facts, launching, marks, manual steps and
//! shutdown. Every call goes to the compositor thread over the command channel.

use std::sync::Arc;

use fakeland_trace::{Environment, Event};
use fakeland_types::{OutputId, SharedClock};

use super::{SessionError, SessionInner};
use crate::config::SessionConfig;
use crate::launch::{App, LaunchSpec};
use crate::server::ClientInfo;
use crate::util::block_on;

/// A running session. Clones share the session, and dropping the last clone shuts it down.
#[derive(Debug, Clone)]
pub struct Session {
    pub(crate) inner: Arc<SessionInner>,
}

impl Session {
    pub(crate) fn from_inner(inner: Arc<SessionInner>) -> Self {
        Self { inner }
    }

    /// Returns the socket name for `WAYLAND_DISPLAY`.
    pub fn socket_name(&self) -> &str {
        &self.inner.socket_name
    }

    /// Returns the session clock.
    pub fn clock(&self) -> SharedClock {
        self.inner.clock.clone()
    }

    /// Returns the environment of the trace header.
    pub fn environment(&self) -> &Environment {
        &self.inner.environment
    }

    /// Returns the private bus address, when the session has one.
    pub fn dbus_address(&self) -> Option<&str> {
        self.inner.dbus_address.as_deref()
    }

    /// Returns the config the session started with.
    pub fn config(&self) -> &SessionConfig {
        &self.inner.config
    }

    /// Returns every connected client with its pid and launched ancestor.
    pub async fn clients(&self) -> Result<Vec<ClientInfo>, SessionError> {
        self.inner
            .call(|state| {
                let mut list: Vec<ClientInfo> = state.clients.list.values().copied().collect();
                list.sort_by_key(|c| c.id);
                list
            })
            .await
    }

    /// Returns every connected client on the calling thread.
    pub fn blocking_clients(&self) -> Result<Vec<ClientInfo>, SessionError> {
        block_on(self.clients())
    }

    /// Launches an app.
    pub async fn launch(&self, spec: LaunchSpec) -> Result<App, SessionError> {
        let shared = self
            .inner
            .call(move |state| state.spawn_app(&spec))
            .await??;
        Ok(App::new(shared, Arc::downgrade(&self.inner)))
    }

    /// Launches an app on the calling thread.
    pub fn blocking_launch(&self, spec: LaunchSpec) -> Result<App, SessionError> {
        block_on(self.launch(spec))
    }

    /// Records a user mark in the trace.
    pub async fn mark(&self, label: impl Into<String>) -> Result<(), SessionError> {
        let label = label.into();
        self.inner
            .call(move |state| {
                state.emit(Event::UserMark { label });
            })
            .await
    }

    /// Records a user mark on the calling thread.
    pub fn blocking_mark(&self, label: impl Into<String>) -> Result<(), SessionError> {
        block_on(self.mark(label))
    }

    /// Steps a manual output by `n` refreshes. `None` selects the primary output.
    pub async fn step(&self, output: Option<OutputId>, n: u32) -> Result<(), SessionError> {
        self.inner
            .call(move |state| {
                let id =
                    state
                        .outputs
                        .resolve(output)
                        .ok_or(crate::outputs::OutputError::NoOutput(
                            output.unwrap_or(OutputId(0)),
                        ))?;
                state.step_output(id, n)
            })
            .await??;
        Ok(())
    }

    /// Steps a manual output on the calling thread.
    pub fn blocking_step(&self, output: Option<OutputId>, n: u32) -> Result<(), SessionError> {
        block_on(self.step(output, n))
    }

    /// Shuts the session down: children end, the socket and the private bus go away.
    pub async fn shutdown(&self) -> Result<(), SessionError> {
        let result = self.inner.call(|state| state.shutdown()).await;
        match result {
            Ok(()) | Err(SessionError::Closed) => Ok(()),
            Err(e) => Err(e),
        }
    }

    /// Shuts the session down on the calling thread and waits for the compositor thread.
    pub fn blocking_shutdown(&self) -> Result<(), SessionError> {
        block_on(self.shutdown())?;
        self.inner.stop();
        Ok(())
    }
}
