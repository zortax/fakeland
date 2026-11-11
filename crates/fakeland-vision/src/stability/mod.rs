//! Content stability: a fast hash of a frame region and a tracker that reports when successive
//! frames stay equal.

mod hash;
mod tracker;

pub use hash::region_hash;
pub use tracker::{StabilityConfig, StabilityStatus, StabilityTracker};
