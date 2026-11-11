//! The public session API: start, the `Session` handle and its calls. The API is async and runtime
//! agnostic; each call has a `blocking_*` variant.

mod a11y;
mod capture;
mod clipboard;
mod commits;
mod error;
mod frames;
mod handle;
mod inner;
mod input;
mod outputs;
mod probes;
mod reactor;
mod resources;
mod scenario;
mod start;
mod trace;
mod viewer;
mod windows;

pub use error::{SessionError, StartError};
pub use handle::Session;
pub use inner::{Command, SessionInner};
pub use scenario::RunHandle;
pub use start::{Fakeland, FakelandBuilder};

pub use crate::shell::WindowOp;
