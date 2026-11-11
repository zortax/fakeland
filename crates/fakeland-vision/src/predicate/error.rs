//! Errors of predicate evaluation.

use fakeland_types::{Physical, Rect};
use thiserror::Error;

use super::ImageHandle;
use crate::diff::DiffError;

/// An error from evaluating a predicate.
#[derive(Clone, Copy, Debug, PartialEq, Error)]
pub enum PredicateError {
    /// No image is stored under the handle.
    #[error("no image for handle {0:?}")]
    UnknownImage(ImageHandle),
    /// The point lies outside the frame.
    #[error("point ({x}, {y}) lies outside the frame")]
    OutsideFrame {
        /// Horizontal coordinate.
        x: u32,
        /// Vertical coordinate.
        y: u32,
    },
    /// The region and the frame do not overlap.
    #[error("region {0:?} does not overlap the frame")]
    EmptyRegion(Option<Rect<Physical>>),
    /// The reference image matches neither the frame size nor the region size.
    #[error("reference size {width}x{height} matches neither the frame nor the region")]
    ReferenceSize {
        /// Width of the reference.
        width: u32,
        /// Height of the reference.
        height: u32,
    },
    /// The frame comparison failed.
    #[error(transparent)]
    Diff(#[from] DiffError),
}
