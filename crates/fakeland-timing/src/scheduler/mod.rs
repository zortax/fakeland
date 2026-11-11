//! The per-output frame scheduler. It owns the refresh model of one output, the queues of its
//! surfaces and the armed frame callbacks, and turns deadlines into ordered actions.

mod advance;
mod control;
mod core;
mod deadline;
mod error;
mod frame;
mod grid_queries;
mod mode_state;

pub use core::Scheduler;
pub use error::ScheduleError;
