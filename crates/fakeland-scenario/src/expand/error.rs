//! Errors of input actions that do not expand.

use crate::MotionError;

/// An input action with values that do not expand.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum ExpandError {
    /// An event rate that is not positive and finite.
    #[error("event rate {0} Hz must be positive and finite")]
    Rate(f64),
    /// A negative duration.
    #[error("{0} must not be negative")]
    NegativeDuration(&'static str),
    /// A count of zero.
    #[error("{0} must be at least 1")]
    Count(&'static str),
    /// A typing speed that is not positive and finite.
    #[error("typing speed {0} wpm must be positive and finite")]
    Wpm(f64),
    /// A typing jitter outside `0..1`.
    #[error("typing jitter {0} must lie in 0..1")]
    Jitter(f64),
    /// A value that is not finite.
    #[error("{0} is not a finite number")]
    NotFinite(&'static str),
    /// A pinch scale that is not positive.
    #[error("pinch scale {0} must be positive")]
    Scale(f64),
    /// A gesture with too few fingers.
    #[error("{gesture} needs at least {min} fingers, got {fingers}")]
    Fingers {
        /// The gesture.
        gesture: &'static str,
        /// The minimum.
        min: u32,
        /// The given count.
        fingers: u32,
    },
    /// A wheel detent that is not positive.
    #[error("wheel detent {0} px must be positive")]
    Detent(f64),
    /// A window size that is not positive.
    #[error("window size {w}x{h} must be positive")]
    Size {
        /// Width.
        w: i32,
        /// Height.
        h: i32,
    },
    /// A path, easing or velocity curve that cannot be evaluated.
    #[error(transparent)]
    Motion(#[from] MotionError),
}
