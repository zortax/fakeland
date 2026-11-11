//! The smoothness score of a per-refresh displacement series.

/// Scores how even a per-refresh displacement series is, from 0 to 100. The score is
/// `100 * (1 - mean |d[r] - d[r-1]| / mean |d[r]|)`, clamped at 0: an even scroll scores 100, and
/// a refresh that repeats a frame and the catch-up after it lower the score. Returns `None` for
/// fewer than two refreshes or no motion.
pub fn smoothness_score(per_refresh: &[f64]) -> Option<f64> {
    if per_refresh.len() < 2 {
        return None;
    }
    let mean_abs = per_refresh.iter().map(|d| d.abs()).sum::<f64>() / per_refresh.len() as f64;
    if mean_abs <= f64::EPSILON {
        return None;
    }
    let changes: Vec<f64> = per_refresh
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .collect();
    let mean_change = changes.iter().sum::<f64>() / changes.len() as f64;
    Some((100.0 * (1.0 - mean_change / mean_abs)).max(0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn even_motion_scores_full() {
        assert_eq!(smoothness_score(&[20.0; 30]), Some(100.0));
        assert_eq!(smoothness_score(&[0.0; 30]), None);
        assert_eq!(smoothness_score(&[1.0]), None);
    }

    #[test]
    fn a_repeated_frame_costs_points() {
        let mut d = vec![20.0; 60];
        d[30] = 0.0;
        d[31] = 40.0;
        let s = smoothness_score(&d).unwrap();
        // Changes 20, 40 and 20 over 59 pairs, mean motion 20.
        let want = 100.0 * (1.0 - (80.0 / 59.0) / 20.0);
        assert!((s - want).abs() < 1e-9, "{s}");
    }
}
