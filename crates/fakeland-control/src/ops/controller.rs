//! The controller: a session plus the registries that outlive single requests, such as launched
//! apps and runs. The socket server and the MCP server both drive a session through it.

use std::sync::{Arc, Mutex, MutexGuard};

use fakeland::Session;

use super::apps::AppRegistry;
use super::runs::RunRegistry;
use crate::methods::{MarkParams, NoParams, SessionInfo, StepParams};
use crate::wire::PROTOCOL_VERSION;

use super::OpError;

/// A session with its control state. Clones share the state.
#[derive(Debug, Clone)]
pub struct Controller {
    pub(super) inner: Arc<Inner>,
}

#[derive(Debug)]
pub(super) struct Inner {
    pub(super) session: Session,
    pub(super) apps: Mutex<AppRegistry>,
    pub(super) runs: Arc<Mutex<RunRegistry>>,
    pub(super) captures: Mutex<Vec<u64>>,
    pub(super) a11y: Mutex<Option<fakeland::Accessibility>>,
}

impl Controller {
    /// Wraps a running session.
    pub fn new(session: Session) -> Self {
        Self {
            inner: Arc::new(Inner {
                session,
                apps: Mutex::new(AppRegistry::default()),
                runs: Arc::new(Mutex::new(RunRegistry::default())),
                captures: Mutex::new(Vec::new()),
                a11y: Mutex::new(None),
            }),
        }
    }

    /// Returns the session.
    pub fn session(&self) -> &Session {
        &self.inner.session
    }

    pub(super) fn apps(&self) -> MutexGuard<'_, AppRegistry> {
        self.inner.apps.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Returns facts about the session.
    pub fn info(&self, _: NoParams) -> SessionInfo {
        let s = self.session();
        SessionInfo {
            protocol: PROTOCOL_VERSION,
            wayland_display: s.socket_name().to_owned(),
            runtime_dir: std::env::var("XDG_RUNTIME_DIR").ok(),
            dbus_address: s.dbus_address().map(str::to_owned),
            server_pid: std::process::id(),
            environment: s.environment().clone(),
            config: s.config().clone(),
        }
    }

    /// Returns the compositor lateness so far.
    pub async fn validity(&self, _: NoParams) -> Result<fakeland::RunValidity, OpError> {
        Ok(self.session().validity().await?)
    }

    /// Clears the lateness record.
    pub async fn reset_validity(&self, _: NoParams) -> Result<(), OpError> {
        Ok(self.session().reset_validity().await?)
    }

    /// Records a user mark.
    pub async fn mark(&self, p: MarkParams) -> Result<(), OpError> {
        Ok(self.session().mark(p.label).await?)
    }

    /// Steps a manual output.
    pub async fn step(&self, p: StepParams) -> Result<(), OpError> {
        Ok(self.session().step(p.output, p.n).await?)
    }

    /// Shuts the session down.
    pub async fn shutdown(&self, _: NoParams) -> Result<(), OpError> {
        Ok(self.session().shutdown().await?)
    }
}
