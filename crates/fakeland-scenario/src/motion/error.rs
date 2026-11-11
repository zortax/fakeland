//! Errors of motion curves that cannot be evaluated.

/// A path, easing or velocity curve that cannot be evaluated.
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum MotionError {
    /// The points of a path use different coordinate frames.
    #[error("the points of a path use different coordinate frames; resolve them to one frame")]
    MixedFrames,
    /// A polyline without points.
    #[error("a polyline needs at least one point")]
    EmptyPath,
    /// A cubic easing with control x values outside `0..=1`.
    #[error("cubic easing x values {x1} and {x2} must lie in 0..=1")]
    CubicRange {
        /// The first control x value.
        x1: f64,
        /// The second control x value.
        x2: f64,
    },
    /// Velocity keyframes that are empty, unsorted or outside `0..=1`.
    #[error("velocity keyframes must be non-empty, sorted and lie in 0..=1")]
    Keyframes,
    /// A value that is not finite.
    #[error("{0} is not a finite number")]
    NotFinite(&'static str),
}
