//! Percentile bootstrap confidence intervals with a seeded generator.

use rand::rngs::Xoshiro256PlusPlus;
use rand::{RngExt, SeedableRng};
use serde::{Deserialize, Serialize};

use crate::stats::percentile::percentile_sorted;

/// A statistic with its bootstrap confidence interval.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfidenceInterval {
    /// The statistic of the full sample.
    pub estimate: f64,
    /// The lower bound.
    pub lo: f64,
    /// The upper bound.
    pub hi: f64,
    /// The confidence level in `0..1`, for example `0.95`.
    pub confidence: f64,
}

impl ConfidenceInterval {
    /// Computes the statistic and its interval. See [`bootstrap_ci`].
    pub fn bootstrap(
        values: &[f64],
        statistic: impl Fn(&[f64]) -> f64,
        iterations: usize,
        confidence: f64,
        seed: u64,
    ) -> Option<Self> {
        let (lo, hi) = bootstrap_ci(values, &statistic, iterations, confidence, seed)?;
        Some(Self {
            estimate: statistic(values),
            lo,
            hi,
            confidence,
        })
    }

    /// Returns the width of the interval.
    pub fn width(&self) -> f64 {
        self.hi - self.lo
    }
}

/// Returns the percentile bootstrap interval of a statistic.
///
/// Each of `iterations` resamples draws `values.len()` values with replacement. The bounds are the
/// `(1 - confidence) / 2` and `(1 + confidence) / 2` quantiles of the resampled statistics. The
/// same seed gives the same interval. Returns `None` for no values, zero iterations, a
/// confidence outside `0..1`, or a statistic without finite results.
pub fn bootstrap_ci(
    values: &[f64],
    statistic: impl Fn(&[f64]) -> f64,
    iterations: usize,
    confidence: f64,
    seed: u64,
) -> Option<(f64, f64)> {
    if values.is_empty() || iterations == 0 || !(confidence > 0.0 && confidence < 1.0) {
        return None;
    }
    let mut rng = Xoshiro256PlusPlus::seed_from_u64(seed);
    let mut sample = vec![0.0; values.len()];
    let mut stats = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        for slot in &mut sample {
            *slot = values[rng.random_range(0..values.len())];
        }
        let s = statistic(&sample);
        if s.is_finite() {
            stats.push(s);
        }
    }
    stats.sort_by(f64::total_cmp);
    let alpha = (1.0 - confidence) / 2.0;
    let lo = percentile_sorted(&stats, alpha * 100.0)?;
    let hi = percentile_sorted(&stats, (1.0 - alpha) * 100.0)?;
    Some((lo, hi))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mean(v: &[f64]) -> f64 {
        v.iter().sum::<f64>() / v.len() as f64
    }

    #[test]
    fn interval_holds_the_mean_and_repeats_with_the_seed() {
        let values: Vec<f64> = (0..200).map(|i| f64::from(i % 20)).collect();
        let a = bootstrap_ci(&values, mean, 2000, 0.95, 7).unwrap();
        let b = bootstrap_ci(&values, mean, 2000, 0.95, 7).unwrap();
        assert_eq!(a, b);
        assert!(a.0 < 9.5 && 9.5 < a.1);
        // The standard error of the mean is about 5.77 / sqrt(200) = 0.41.
        let width = a.1 - a.0;
        assert!(width > 1.2 && width < 2.0, "width {width}");
    }

    #[test]
    fn constant_sample_has_zero_width() {
        let ci = ConfidenceInterval::bootstrap(&[3.0; 10], mean, 100, 0.9, 1).unwrap();
        assert_eq!((ci.estimate, ci.lo, ci.hi), (3.0, 3.0, 3.0));
        assert_eq!(ci.width(), 0.0);
    }

    #[test]
    fn invalid_inputs() {
        assert!(bootstrap_ci(&[], mean, 10, 0.95, 0).is_none());
        assert!(bootstrap_ci(&[1.0], mean, 0, 0.95, 0).is_none());
        assert!(bootstrap_ci(&[1.0], mean, 10, 1.0, 0).is_none());
        assert!(bootstrap_ci(&[1.0], |_| f64::NAN, 10, 0.5, 0).is_none());
    }
}
