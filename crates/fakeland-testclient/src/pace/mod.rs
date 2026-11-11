//! Frame pacing: the present modes and the fixed-rate tick grid.

mod pacer;
mod ticks;

pub use pacer::{Decision, Pacer, PresentMode};
pub use ticks::Ticks;
