//! Confidence of correlation peaks and the rules that reject unreliable estimates.

use super::peak::Peak;
use super::{MotionConfig, MotionRejection};

/// A phase correlation peak this many times the surface RMS or more counts at full strength.
const STRONG_PSR: f64 = 25.0;
/// A phase correlation peak below this many times the surface RMS is noise. Uncorrelated content
/// peaks near five.
const WEAK_PSR: f64 = 8.0;
/// A peak whose runner-up reaches this fraction of its height is ambiguous.
const AMBIGUOUS_RATIO: f64 = 0.75;

/// Returns the ratio of a peak to the RMS of its surface.
fn psr(p: &Peak) -> f64 {
    if p.rms > 0.0 { p.value / p.rms } else { 0.0 }
}

/// Returns the confidence of a phase correlation peak: its distinctness against the runner-up
/// times its strength against the surface RMS.
pub(crate) fn peak_confidence(p: &Peak) -> f64 {
    if p.value <= 0.0 {
        return 0.0;
    }
    let distinct = (1.0 - p.second.max(0.0) / p.value).clamp(0.0, 1.0);
    let strength = ((psr(p) - WEAK_PSR) / (STRONG_PSR - WEAK_PSR)).clamp(0.0, 1.0);
    distinct * strength
}

/// Accepts a phase correlation peak or names the reason to reject it.
pub(crate) fn judge_peak(p: &Peak, config: &MotionConfig) -> Result<f64, MotionRejection> {
    let confidence = peak_confidence(p);
    if psr(p) < WEAK_PSR {
        return Err(MotionRejection::LowConfidence { confidence });
    }
    if p.second >= AMBIGUOUS_RATIO * p.value {
        return Err(MotionRejection::Ambiguous { confidence });
    }
    if confidence < config.min_confidence {
        return Err(MotionRejection::LowConfidence { confidence });
    }
    Ok(confidence)
}

/// A cross-correlation gap to the runner-up of this size or more counts as fully distinct.
const DISTINCT_GAP: f64 = 0.2;
/// A strong cross-correlation maximum with a gap below this size is ambiguous.
const AMBIGUOUS_GAP: f64 = 0.04;

/// Returns the confidence of a normalized cross-correlation maximum `best` with runner-up local
/// maximum `second`: the maximum times its distinctness.
pub(crate) fn ncc_confidence(best: f64, second: f64) -> f64 {
    let distinct = ((best - second) / DISTINCT_GAP).clamp(0.0, 1.0);
    best.clamp(0.0, 1.0) * distinct
}

/// Accepts a cross-correlation maximum or names the reason to reject it.
pub(crate) fn judge_ncc(
    best: f64,
    second: f64,
    config: &MotionConfig,
) -> Result<f64, MotionRejection> {
    let confidence = ncc_confidence(best, second);
    if best >= 0.5 && best - second < AMBIGUOUS_GAP {
        return Err(MotionRejection::Ambiguous { confidence });
    }
    if confidence < config.min_confidence {
        return Err(MotionRejection::LowConfidence { confidence });
    }
    Ok(confidence)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peak(value: f64, second: f64) -> Peak {
        Peak {
            dx: 0.0,
            dy: 0.0,
            value,
            second,
            rms: 0.01,
        }
    }

    #[test]
    fn peak_rules() {
        let c = MotionConfig::default();
        assert!(judge_peak(&peak(0.5, 0.05), &c).is_ok());
        assert!(matches!(
            judge_peak(&peak(0.5, 0.45), &c),
            Err(MotionRejection::Ambiguous { .. })
        ));
        assert!(matches!(
            judge_peak(&peak(0.05, 0.0), &c),
            Err(MotionRejection::LowConfidence { .. })
        ));
        assert!(peak_confidence(&peak(0.5, 0.0)) == 1.0);
    }

    #[test]
    fn ncc_rules() {
        let c = MotionConfig::default();
        assert!(judge_ncc(0.99, 0.6, &c).is_ok());
        assert!(matches!(
            judge_ncc(0.99, 0.98, &c),
            Err(MotionRejection::Ambiguous { .. })
        ));
        assert!(matches!(
            judge_ncc(0.2, 0.1, &c),
            Err(MotionRejection::LowConfidence { .. })
        ));
    }
}
