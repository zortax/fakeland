//! Compositor-side waiting: pending waits with deadlines, in-thread record observers, settle
//! detection, regions of interest, pixel conditions and the scenario waits built on them.

mod observers;
mod pixel_watch;
mod pixels;
mod roi;
mod settle;
mod start;
mod waiters;

pub use observers::{Observe, ObserverId, Observers};
pub use pixels::{Condition, ConditionError, changed_fraction};
pub use start::{WaitError, WaitOutput, mark_name};
pub use waiters::Waiters;
