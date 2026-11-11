//! The accept loop of the server.

use std::os::unix::net::UnixListener;
use std::sync::Arc;

use super::connection::serve_connection;
use super::peer::same_user;
use super::stop::StopFlag;
use crate::ops::Controller;

/// Accepts connections until the stop flag is set. Each connection of the same user gets its own
/// thread; connections of other users close at once.
pub fn accept_loop(listener: UnixListener, controller: Controller, stop: Arc<StopFlag>) {
    for stream in listener.incoming() {
        if stop.is_set() {
            break;
        }
        let stream = match stream {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("control accept failed: {e}");
                continue;
            }
        };
        if !same_user(&stream) {
            tracing::warn!("control connection from another user refused");
            continue;
        }
        let controller = controller.clone();
        let stop = Arc::clone(&stop);
        let spawned = std::thread::Builder::new()
            .name("fakeland-ctl-conn".into())
            .spawn(move || serve_connection(stream, controller, stop));
        if let Err(e) = spawned {
            tracing::warn!("cannot start a connection thread: {e}");
        }
    }
}
