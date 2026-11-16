//! The session lock: the lock state, one lock surface per output that covers it, input and
//! keyboard focus that reach only lock surfaces while the session is locked, and unlock.

mod lifecycle;
mod state;

pub use state::{LockRecord, SessionLock};
