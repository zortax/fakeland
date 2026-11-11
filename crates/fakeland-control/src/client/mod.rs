//! The typed control client. One reader thread demultiplexes responses and events, so calls can
//! run in parallel from several threads or tasks. Calls are async and runtime agnostic, with
//! `blocking_*` variants.

mod connection;
mod error;
mod events;
mod subscription;

pub use connection::ControlClient;
pub use error::ClientError;
pub use subscription::{ClientSubscription, SubEvent};
