//! Event ticks of a timed program: `N + 1` evenly spaced offsets that include both ends.

use fakeland_types::{Dur, Nanos};

use super::ExpandError;

/// Checks a rate and a duration.
pub(super) fn check(duration: Dur, rate_hz: f64, what: &'static str) -> Result<(), ExpandError> {
    if !rate_hz.is_finite() || rate_hz <= 0.0 {
        return Err(ExpandError::Rate(rate_hz));
    }
    if duration.0.is_negative() {
        return Err(ExpandError::NegativeDuration(what));
    }
    Ok(())
}

/// Returns `(offset, progress)` for `k = 0..=N`, where `N = max(1, round(duration * rate))` and
/// progress is `k / N`.
pub fn ticks(duration: Dur, rate_hz: f64) -> Result<Vec<(Nanos, f64)>, ExpandError> {
    check(duration, rate_hz, "duration")?;
    let total = duration.0.as_nanos();
    let n = ((total as f64 / 1e9) * rate_hz).round().max(1.0) as i64;
    Ok((0..=n)
        .map(|k| {
            let offset = (total as i128 * k as i128 / n as i128) as i64;
            (Nanos(offset), k as f64 / n as f64)
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticks_include_both_ends() {
        let t = ticks(Dur::from_secs(1), 125.0).unwrap_or_default();
        assert_eq!(t.len(), 126);
        assert_eq!(t[0], (Nanos::ZERO, 0.0));
        assert_eq!(t[125], (Nanos::from_secs(1), 1.0));
        assert_eq!(t[1].0, Nanos::from_millis(8));
        let t = ticks(Dur::ZERO, 60.0).unwrap_or_default();
        assert_eq!(t, vec![(Nanos::ZERO, 0.0), (Nanos::ZERO, 1.0)]);
        assert!(ticks(Dur::from_secs(1), 0.0).is_err());
        assert!(ticks(Dur::from_millis(-1), 60.0).is_err());
    }
}
