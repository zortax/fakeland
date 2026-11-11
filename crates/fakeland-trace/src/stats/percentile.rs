//! Percentiles by linear interpolation between closest ranks.

/// Returns the `p`-th percentile (`0..=100`) of sorted finite values by linear interpolation
/// between closest ranks: the rank is `p / 100 * (n - 1)`. Returns `None` for no values.
pub fn percentile_sorted(sorted: &[f64], p: f64) -> Option<f64> {
    let last = sorted.len().checked_sub(1)?;
    let rank = (p.clamp(0.0, 100.0) / 100.0) * last as f64;
    let lo = rank.floor() as usize;
    let hi = rank.ceil() as usize;
    let frac = rank - lo as f64;
    Some(sorted[lo] + (sorted[hi] - sorted[lo]) * frac)
}

/// Returns the `p`-th percentile (`0..=100`) of the finite values. See [`percentile_sorted`].
pub fn percentile(values: &[f64], p: f64) -> Option<f64> {
    percentile_sorted(&sorted_finite(values), p)
}

/// Returns the finite values in ascending order.
pub(crate) fn sorted_finite(values: &[f64]) -> Vec<f64> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| x.is_finite()).collect();
    v.sort_by(f64::total_cmp);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interpolates_between_ranks() {
        let v = [1.0, 2.0, 3.0, 4.0];
        assert_eq!(percentile(&v, 50.0), Some(2.5));
        assert_eq!(percentile(&v, 0.0), Some(1.0));
        assert_eq!(percentile(&v, 100.0), Some(4.0));
        let p90 = percentile(&v, 90.0).unwrap_or_default();
        assert!((p90 - 3.7).abs() < 1e-12);
    }

    #[test]
    fn ignores_non_finite_and_empty() {
        assert_eq!(percentile(&[], 50.0), None);
        assert_eq!(percentile(&[f64::NAN], 50.0), None);
        assert_eq!(percentile(&[3.0, f64::INFINITY, 1.0], 50.0), Some(2.0));
        assert_eq!(percentile(&[7.0], 99.9), Some(7.0));
    }
}
