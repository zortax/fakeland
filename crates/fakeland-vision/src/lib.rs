//! Frame analysis on captured pixels: change detection in a region, pixel predicates, stability,
//! damage accuracy and content motion estimation.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod damage;
pub mod diff;
pub mod image;
pub mod motion;
pub mod predicate;
pub mod stability;

#[cfg(test)]
mod synth;

pub use damage::{DamageAccuracy, damage_accuracy};
pub use diff::{DiffError, DiffStats, changed, diff};
pub use image::{
    GrayImage, GrayImageF32, Image, ImageError, ImageView, PixelFormat, RowOrder, fourcc, luma,
};
pub use motion::{
    Axis, Motion, MotionConfig, MotionEstimator, MotionRejection, estimate_motion,
    estimate_scroll_1d, estimate_translation,
};
pub use predicate::{
    Evaluation, ImageHandle, ImageSource, ImageStore, NoImages, Predicate, PredicateError, Rgba,
    evaluate,
};
pub use stability::{StabilityConfig, StabilityStatus, StabilityTracker, region_hash};
