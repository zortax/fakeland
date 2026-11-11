//! Frame-rate lows from frame intervals.

use fakeland_types::Nanos;
use serde::{Deserialize, Serialize};

use crate::stats::percentile::{percentile_sorted, sorted_finite};

/// Average frame rate and the low frame rates of the slowest frames.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FpsLows {
    /// The number of intervals.
    pub frames: usize,
    /// Frames over total time: 1000 over the mean interval in milliseconds.
    pub avg_fps: f64,
    /// 1000 over the mean of the slowest 1% of intervals (at least one interval).
    pub low_1pct_fps: f64,
    /// 1000 over the mean of the slowest 0.1% of intervals (at least one interval).
    pub low_01pct_fps: f64,
    /// 1000 over the 99th percentile interval.
    pub p1_fps: f64,
    /// 1000 over the 99.9th percentile interval.
    pub p01_fps: f64,
}

impl FpsLows {
    /// Computes the lows from positive frame intervals in milliseconds. Other values are skipped.
    /// Returns `None` without a positive interval.
    pub fn from_intervals_ms(intervals: &[f64]) -> Option<Self> {
        let positive: Vec<f64> = intervals.iter().copied().filter(|v| *v > 0.0).collect();
        let sorted = sorted_finite(&positive);
        if sorted.is_empty() {
            return None;
        }
        let n = sorted.len();
        let worst_mean = |fraction: f64| {
            let k = ((n as f64 * fraction).ceil() as usize).clamp(1, n);
            sorted[n - k..].iter().sum::<f64>() / k as f64
        };
        let mean = sorted.iter().sum::<f64>() / n as f64;
        let fps = |ms: f64| 1000.0 / ms;
        Some(Self {
            frames: n,
            avg_fps: fps(mean),
            low_1pct_fps: fps(worst_mean(0.01)),
            low_01pct_fps: fps(worst_mean(0.001)),
            p1_fps: fps(percentile_sorted(&sorted, 99.0)?),
            p01_fps: fps(percentile_sorted(&sorted, 99.9)?),
        })
    }

    /// Computes the lows from frame intervals.
    pub fn from_intervals(intervals: impl IntoIterator<Item = Nanos>) -> Option<Self> {
        let ms: Vec<f64> = intervals.into_iter().map(Nanos::as_millis_f64).collect();
        Self::from_intervals_ms(&ms)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_lows() {
        // 990 frames at 10 ms and 10 frames at 50 ms.
        let mut v = vec![10.0; 990];
        v.extend([50.0; 10]);
        let l = FpsLows::from_intervals_ms(&v).unwrap();
        assert_eq!(l.frames, 1000);
        assert!((l.avg_fps - 1000.0 / 10.4).abs() < 1e-9);
        assert!((l.low_1pct_fps - 20.0).abs() < 1e-9);
        assert!((l.low_01pct_fps - 20.0).abs() < 1e-9);
        // The 99th percentile sits between the 10 ms and 50 ms groups.
        assert!((l.p1_fps - 1000.0 / 10.4).abs() < 1e-9);
        assert!((l.p01_fps - 20.0).abs() < 1e-9);
    }

    #[test]
    fn steady_rate() {
        let l = FpsLows::from_intervals(vec![Nanos(16_666_667); 100]).unwrap();
        assert!((l.avg_fps - 60.0).abs() < 1e-3);
        assert!((l.low_1pct_fps - 60.0).abs() < 1e-3);
        assert!(FpsLows::from_intervals_ms(&[0.0, -1.0]).is_none());
    }
}
