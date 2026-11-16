//! The session lock: the lock state, one lock surface per output that covers it, input and
//! keyboard focus that reach only lock surfaces while the session is locked, and unlock.

mod hold;
mod lifecycle;
mod route;
mod state;

pub use state::{LockRecord, SessionLock};
