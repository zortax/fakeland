//! The private fakeland_marks_v1 Wayland protocol. The `server` feature adds the bindings for
//! the compositor and the `client` feature adds the bindings for apps. The wire encoding of ids,
//! timestamps and names is always present, so both sides encode and check values the same way.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "client")]
pub mod client;
mod global;
#[cfg(feature = "server")]
pub mod server;
pub mod wire;

pub use global::{MANAGER_INTERFACE, MANAGER_VERSION};
