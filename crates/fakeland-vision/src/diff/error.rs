//! Errors of frame comparison.

use thiserror::Error;

use crate::image::ImageView;

/// An error from comparing two frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum DiffError {
    /// The frames have different sizes.
    #[error("frame sizes differ: {a_width}x{a_height} and {b_width}x{b_height}")]
    SizeMismatch {
        /// Width of the first frame.
        a_width: u32,
        /// Height of the first frame.
        a_height: u32,
        /// Width of the second frame.
        b_width: u32,
        /// Height of the second frame.
        b_height: u32,
    },
}

/// Returns an error when the two views differ in size.
pub(crate) fn check_same_size(a: &ImageView<'_>, b: &ImageView<'_>) -> Result<(), DiffError> {
    if a.same_size(b) {
        Ok(())
    } else {
        Err(DiffError::SizeMismatch {
            a_width: a.width(),
            a_height: a.height(),
            b_width: b.width(),
            b_height: b.height(),
        })
    }
}
