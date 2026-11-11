//! The control socket server. It accepts connections on a unix socket and runs each request on
//! its own thread, so a long wait never blocks other requests. Every operation runs on the
//! compositor thread through the session; the server measures nothing itself.

mod connection;
mod dispatch;
mod handle;
mod listener;
mod peer;
mod stop;
mod subscriptions;
mod writer;

pub use handle::{ControlServer, ServeError, StopHandle, serve, serve_session};
