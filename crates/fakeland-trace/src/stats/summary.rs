//! Descriptive summaries of samples.

use fakeland_types::Nanos;
use serde::{Deserialize, Serialize};

use crate::stats::percentile::{percentile_sorted, sorted_finite};

/// Count, moments, extremes and percentiles of a sample.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    /// The number of finite values.
    pub count: usize,
    /// The arithmetic mean.
    pub mean: f64,
    /// The sample standard deviation (divisor `n - 1`). Zero for one value.
    pub stddev: f64,
    /// The smallest value.
    pub min: f64,
    /// The largest value.
    pub max: f64,
    /// The median.
    pub p50: f64,
    /// The 90th percentile.
    pub p90: f64,
    /// The 95th percentile.
    pub p95: f64,
    /// The 99th percentile.
    pub p99: f64,
    /// The 99.9th percentile.
    pub p999: f64,
}

impl Summary {
    /// Summarizes the finite values. Returns `None` when no value is finite.
    pub fn of(values: &[f64]) -> Option<Self> {
        let sorted = sorted_finite(values);
        let count = sorted.len();
        if count == 0 {
            return None;
        }
        let mean = sorted.iter().sum::<f64>() / count as f64;
        let var = if count > 1 {
            sorted.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (count - 1) as f64
        } else {
            0.0
        };
        let p = |q| percentile_sorted(&sorted, q).unwrap_or(mean);
        Some(Self {
            count,
            mean,
            stddev: var.sqrt(),
            min: sorted[0],
            max: sorted[count - 1],
            p50: p(50.0),
            p90: p(90.0),
            p95: p(95.0),
            p99: p(99.0),
            p999: p(99.9),
        })
    }

    /// Summarizes spans in milliseconds.
    pub fn of_millis(spans: impl IntoIterator<Item = Nanos>) -> Option<Self> {
        let v: Vec<f64> = spans.into_iter().map(Nanos::as_millis_f64).collect();
        Self::of(&v)
    }

    /// Returns the coefficient of variation: stddev over mean. Zero for a zero mean.
    pub fn cv(&self) -> f64 {
        if self.mean == 0.0 {
            0.0
        } else {
            self.stddev / self.mean
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn known_values() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        let s = Summary::of(&v).unwrap();
        assert_eq!(s.count, 100);
        assert!(close(s.mean, 50.5));
        assert!(close(s.stddev, 29.011_491_975_882_016));
        assert!(close(s.min, 1.0));
        assert!(close(s.max, 100.0));
        assert!(close(s.p50, 50.5));
        assert!(close(s.p90, 90.1));
        assert!(close(s.p95, 95.05));
        assert!(close(s.p99, 99.01));
        assert!(close(s.p999, 99.901));
    }

    #[test]
    fn single_and_empty() {
        let s = Summary::of(&[4.0]).unwrap();
        assert_eq!(s.stddev, 0.0);
        assert_eq!(s.p99, 4.0);
        assert!(Summary::of(&[]).is_none());
        assert!(Summary::of(&[f64::NAN]).is_none());
    }

    #[test]
    fn millis_of_spans() {
        let s = Summary::of_millis([Nanos::from_millis(2), Nanos::from_millis(4)]).unwrap();
        assert!(close(s.mean, 3.0));
        assert!(close(s.cv(), s.stddev / 3.0));
    }

    #[test]
    fn serde_round_trip() {
        let s = Summary::of(&[1.0, 2.0, 3.0]).unwrap();
        let json = serde_json::to_string(&s).unwrap();
        let back: Summary = serde_json::from_str(&json).unwrap();
        assert_eq!(s, back);
    }
}
