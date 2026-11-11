//! Injected load: render delays, periodic jank and CPU burn per frame.

mod plan;
mod wait;

pub use plan::{DelayKind, LoadPlan};
pub use wait::{burn, wait_until};
