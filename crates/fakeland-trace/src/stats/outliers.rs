//! Outlier filtering by the median absolute deviation.

use serde::{Deserialize, Serialize};

use crate::stats::percentile::{percentile_sorted, sorted_finite};

/// The scale that makes the MAD a consistent estimator of the standard deviation of a normal
/// distribution.
const MAD_SCALE: f64 = 1.482_602_218_505_602;

/// The result of a MAD outlier filter.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MadFilter {
    /// The median of the finite values.
    pub median: f64,
    /// The scaled median absolute deviation.
    pub mad: f64,
    /// The threshold in scaled MADs.
    pub k: f64,
    /// The kept values in input order.
    pub kept: Vec<f64>,
    /// The input indices of the outliers, non-finite values included.
    pub outliers: Vec<usize>,
}

/// Returns the median of the finite values.
pub fn median(values: &[f64]) -> Option<f64> {
    percentile_sorted(&sorted_finite(values), 50.0)
}

/// Splits values into kept values and outliers. A value is an outlier when it lies more than `k`
/// scaled MADs from the median, or when it is not finite. A MAD of zero keeps every finite value.
/// Returns `None` without a finite value.
pub fn mad_filter(values: &[f64], k: f64) -> Option<MadFilter> {
    let med = median(values)?;
    let deviations: Vec<f64> = values
        .iter()
        .filter(|v| v.is_finite())
        .map(|v| (v - med).abs())
        .collect();
    let mad = median(&deviations)? * MAD_SCALE;
    let mut kept = Vec::new();
    let mut outliers = Vec::new();
    for (i, v) in values.iter().enumerate() {
        let out = !v.is_finite() || (mad > 0.0 && (v - med).abs() > k * mad);
        if out {
            outliers.push(i);
        } else {
            kept.push(*v);
        }
    }
    Some(MadFilter {
        median: med,
        mad,
        k,
        kept,
        outliers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_the_spike() {
        let v = [10.0, 11.0, 9.0, 10.5, 9.5, 100.0, 10.0, f64::NAN];
        let f = mad_filter(&v, 3.5).unwrap();
        assert_eq!(f.median, 10.0);
        assert!((f.mad - 0.5 * MAD_SCALE).abs() < 1e-9);
        assert_eq!(f.outliers, vec![5, 7]);
        assert_eq!(f.kept, vec![10.0, 11.0, 9.0, 10.5, 9.5, 10.0]);
    }

    #[test]
    fn zero_mad_keeps_everything() {
        let f = mad_filter(&[5.0, 5.0, 5.0, 6.0], 3.0).unwrap();
        assert_eq!(f.mad, 0.0);
        assert!(f.outliers.is_empty());
        assert!(mad_filter(&[], 3.0).is_none());
        assert_eq!(median(&[3.0, 1.0, 2.0]), Some(2.0));
    }
}
