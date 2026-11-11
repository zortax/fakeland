//! The motion estimator: reusable FFT plans and buffers for repeated estimates on frame streams.

use std::cell::RefCell;

use super::phase::PhaseCorrelator;

/// Reusable state of motion estimation. One estimator serves frames of any size; buffers grow to
/// the largest region seen and stay allocated.
#[derive(Debug, Default)]
pub struct MotionEstimator {
    pub(crate) coarse: PhaseCorrelator,
    pub(crate) fine: PhaseCorrelator,
    pub(crate) gray_a: Vec<f32>,
    pub(crate) gray_b: Vec<f32>,
    pub(crate) patch_a: Vec<f32>,
    pub(crate) patch_b: Vec<f32>,
    pub(crate) acc: Vec<u32>,
    pub(crate) profile_a: Vec<f32>,
    pub(crate) profile_b: Vec<f32>,
    pub(crate) scores: Vec<f64>,
    pub(crate) prefix: Vec<f64>,
}

impl MotionEstimator {
    /// Builds an estimator with empty buffers.
    pub fn new() -> Self {
        Self::default()
    }
}

thread_local! {
    static ESTIMATOR: RefCell<MotionEstimator> = RefCell::new(MotionEstimator::new());
}

/// Runs a closure with the estimator of the current thread.
pub(crate) fn with_estimator<T>(f: impl FnOnce(&mut MotionEstimator) -> T) -> T {
    ESTIMATOR.with(|e| f(&mut e.borrow_mut()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimator_moves_between_threads() {
        fn assert_send<T: Send>() {}
        assert_send::<MotionEstimator>();
    }
}
