//! The context log: the events that set the state of outputs, clients, processes, surfaces and
//! windows. A trace slice starts with the context that is live at its first record.

mod log;

pub use log::ContextLog;
