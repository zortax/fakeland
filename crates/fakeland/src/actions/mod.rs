//! Execution of single scenario actions on the compositor thread: input programs on the timer
//! queue, output and window changes, marks, captures, waits, probes, sleeps and the clipboard.
//! Scenarios, triggers, reactors and the session share it.

mod context;
mod error;
mod outcome;
mod perform;
mod program;

pub use context::ActionCtx;
pub use error::ActionError;
pub use outcome::{ActionDone, ActionOutcome};
