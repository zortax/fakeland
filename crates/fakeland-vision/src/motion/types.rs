//! Motion results, the scroll axis, the estimator configuration and the rejection reasons.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// A content displacement between two frames in full-resolution pixels. Content at `(x, y)` in
/// the earlier frame appears at `(x + dx, y + dy)` in the later frame. A page that scrolls down
/// moves its content up, so `dy` is negative.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Motion {
    /// Horizontal displacement.
    pub dx: f64,
    /// Vertical displacement.
    pub dy: f64,
    /// Trust in the estimate, from 0 to 1.
    pub confidence: f64,
}

/// A motion axis.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Axis {
    /// Left and right.
    Horizontal,
    /// Up and down.
    Vertical,
}

/// Settings of motion estimation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MotionConfig {
    /// The largest displacement to search, in full-resolution pixels, per axis.
    pub max_shift: u32,
    /// The coarse pass downscales the region until its longer side fits this many pixels.
    pub coarse_dim: u32,
    /// Side of the full-resolution patch of the refinement pass. Zero skips refinement.
    pub refine_dim: u32,
    /// Estimates below this confidence are rejected.
    pub min_confidence: f64,
    /// Content with a luma standard deviation below this value counts as flat.
    pub flat_std: f64,
    /// A known motion axis. The combined estimator then checks the 2D result against a 1D scroll
    /// estimate along this axis.
    pub axis: Option<Axis>,
}

impl Default for MotionConfig {
    fn default() -> Self {
        Self {
            max_shift: 256,
            coarse_dim: 256,
            refine_dim: 128,
            min_confidence: 0.3,
            flat_std: 1.5,
            axis: None,
        }
    }
}

impl MotionConfig {
    /// Returns the default settings with a search range.
    pub fn with_max_shift(max_shift: u32) -> Self {
        Self {
            max_shift,
            ..Self::default()
        }
    }
}

/// The reason an estimator returns no motion.
#[derive(Clone, Copy, Debug, PartialEq, Error, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MotionRejection {
    /// The region and the frames do not overlap.
    #[error("the region does not overlap the frames")]
    EmptyRegion,
    /// The frames have different sizes.
    #[error("the frames have different sizes")]
    SizeMismatch,
    /// The region is too small to estimate motion.
    #[error("the region is too small")]
    TooSmall,
    /// The content has too little structure.
    #[error("the content is flat")]
    Flat,
    /// Several displacements fit the content about equally well, as with repeating patterns.
    #[error("the displacement is ambiguous (confidence {confidence:.3})")]
    Ambiguous {
        /// Confidence of the best candidate.
        confidence: f64,
    },
    /// The best displacement fits the content poorly.
    #[error("the displacement is unreliable (confidence {confidence:.3})")]
    LowConfidence {
        /// Confidence of the best candidate.
        confidence: f64,
    },
    /// The 2D and 1D estimates disagree.
    #[error("the 2D and 1D estimates disagree")]
    Disagree,
}
