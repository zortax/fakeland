//! The JSON lines event log: event types, the input detail form and the writer.

mod event;
mod input;
mod writer;

pub use event::{CommitInfo, GlobalInfo, LogEvent};
pub use input::InputDetail;
pub use writer::EventLog;
