//! The app-mark server: the manager objects of each client, the double-buffered marks of each
//! surface, input ids sent before injected events, and the trace records of applied marks.

mod commit;
mod server;
mod slot;

pub use server::MarksServer;
pub use slot::{SlotState, with_slot};
