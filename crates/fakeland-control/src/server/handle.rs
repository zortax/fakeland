//! The server handle: start, wait for a stop and stop.

use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use fakeland::Session;
use thiserror::Error;

use super::listener::accept_loop;
use super::stop::StopFlag;
use crate::ops::Controller;
use crate::socket::{BindError, bind, default_socket_path};

/// A server that fails to start.
#[derive(Debug, Error)]
pub enum ServeError {
    /// The socket cannot be bound.
    #[error(transparent)]
    Bind(#[from] BindError),
    /// The accept thread does not start.
    #[error("cannot start the accept thread: {0}")]
    Thread(std::io::Error),
}

/// A running control server. Dropping it stops the server and removes the socket file.
#[derive(Debug)]
pub struct ControlServer {
    path: PathBuf,
    controller: Controller,
    stop: Arc<StopFlag>,
    thread: Option<JoinHandle<()>>,
}

/// Serves a session at a path, or at its default path in `$XDG_RUNTIME_DIR/fakeland/`.
pub fn serve_session(session: Session, path: Option<&Path>) -> Result<ControlServer, ServeError> {
    serve(Controller::new(session), path)
}

/// Serves a controller at a path, or at the default path of its session.
pub fn serve(controller: Controller, path: Option<&Path>) -> Result<ControlServer, ServeError> {
    let path = match path {
        Some(p) => p.to_path_buf(),
        None => default_socket_path(controller.session().socket_name())?,
    };
    let listener = bind(&path)?;
    let stop = Arc::new(StopFlag::default());
    let thread = {
        let controller = controller.clone();
        let stop = Arc::clone(&stop);
        std::thread::Builder::new()
            .name("fakeland-ctl-accept".into())
            .spawn(move || accept_loop(listener, controller, stop))
            .map_err(ServeError::Thread)?
    };
    Ok(ControlServer {
        path,
        controller,
        stop,
        thread: Some(thread),
    })
}

impl ControlServer {
    /// Returns the socket path.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the controller of the served session.
    pub fn controller(&self) -> &Controller {
        &self.controller
    }

    /// Returns true once a client asked for shutdown or the server stopped.
    pub fn is_stopping(&self) -> bool {
        self.stop.is_set()
    }

    /// Blocks until a client asks for shutdown or [`ControlServer::stop`] runs.
    pub fn wait(&self) {
        self.stop.wait();
    }

    /// Blocks until a stop or until the limit passes. Returns true on a stop.
    pub fn wait_for(&self, limit: Duration) -> bool {
        self.stop.wait_for(limit)
    }

    /// Returns a handle that stops the server or waits for its stop from another thread.
    pub fn stop_handle(&self) -> StopHandle {
        StopHandle {
            stop: Arc::clone(&self.stop),
        }
    }

    /// Stops accepting connections and removes the socket file. Open connections end when
    /// their clients close them.
    pub fn stop(&mut self) {
        self.stop.set();
        if let Some(thread) = self.thread.take() {
            let _ = UnixStream::connect(&self.path);
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

impl Drop for ControlServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Stops a server, or waits for its stop, from another thread.
#[derive(Debug, Clone)]
pub struct StopHandle {
    stop: Arc<StopFlag>,
}

impl StopHandle {
    /// Asks the server to stop. [`ControlServer::wait`] returns.
    pub fn stop(&self) {
        self.stop.set();
    }

    /// Returns true once a stop was asked for.
    pub fn is_stopping(&self) -> bool {
        self.stop.is_set()
    }

    /// Blocks until a stop is asked for.
    pub fn wait(&self) {
        self.stop.wait();
    }
}
