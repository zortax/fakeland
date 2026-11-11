//! Content motion between two frames: 2D translation by phase correlation on downscaled luma with
//! full-resolution refinement, 1D scroll by band profile cross-correlation, and a combined
//! estimator that reports confidence and rejects flat or ambiguous content.

#[cfg(test)]
mod bench;
mod combined;
mod confidence;
mod estimator;
mod peak;
mod phase;
mod profile;
mod refine;
mod scroll;
mod sizes;
mod translation;
mod types;

pub use combined::estimate_motion;
pub use estimator::MotionEstimator;
pub use scroll::estimate_scroll_1d;
pub use translation::estimate_translation;
pub use types::{Axis, Motion, MotionConfig, MotionRejection};
