//! The display timing engine: the deadline timer, the scheduled-task queue, wake handling for the
//! output schedulers, execution of scheduler actions and wake lateness.

mod actions;
mod clear;
mod engine;
mod lateness;
mod queue;
mod timer;

pub use clear::MAX_WAITS;
pub use engine::Timers;
pub use lateness::{LatenessTracker, RunValidity};
pub use queue::{TimerId, TimerQueue};
pub use timer::DeadlineTimer;
