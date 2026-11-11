//! The combined motion estimator. It runs 2D phase correlation and, when the motion axis is
//! known, checks it against the 1D profile estimate along that axis.

use fakeland_types::{Physical, Rect};

use super::estimator::with_estimator;
use super::{Axis, Motion, MotionConfig, MotionEstimator, MotionRejection};
use crate::image::ImageView;

/// One estimate wins a disagreement when its confidence is this many times the other's.
const DOMINANCE: f64 = 2.0;

impl MotionEstimator {
    /// Estimates the content motion of a region between two frames.
    ///
    /// Without a known axis the result is the 2D phase correlation estimate. With a known axis the
    /// 1D profile estimate along that axis runs too. When both succeed and agree, the result takes
    /// the 2D displacement and the larger confidence. When they disagree, an estimate with twice
    /// the confidence of the other wins; otherwise the result is [`MotionRejection::Disagree`].
    /// When only one succeeds, it is the result.
    pub fn estimate<'a, 'b>(
        &mut self,
        prev: impl Into<ImageView<'a>>,
        next: impl Into<ImageView<'b>>,
        roi: Option<Rect<Physical>>,
        config: &MotionConfig,
    ) -> Result<Motion, MotionRejection> {
        let (prev, next) = (prev.into(), next.into());
        let planar = self.translation(prev, next, roi, config);
        let Some(axis) = config.axis else {
            return planar;
        };
        let linear = self.scroll_1d(prev, next, roi, axis, config);
        match (planar, linear) {
            (Ok(p), Ok(l)) => reconcile(p, l, axis),
            (Ok(p), Err(_)) => Ok(p),
            (Err(_), Ok(l)) => Ok(l),
            (Err(e), Err(_)) => Err(e),
        }
    }
}

/// Merges a 2D and a 1D estimate along an axis.
fn reconcile(planar: Motion, linear: Motion, axis: Axis) -> Result<Motion, MotionRejection> {
    let (p, l) = match axis {
        Axis::Vertical => (planar.dy, linear.dy),
        Axis::Horizontal => (planar.dx, linear.dx),
    };
    let tolerance = (0.05 * p.abs().max(l.abs())).max(1.0);
    if (p - l).abs() <= tolerance {
        return Ok(Motion {
            confidence: planar.confidence.max(linear.confidence),
            ..planar
        });
    }
    if planar.confidence >= DOMINANCE * linear.confidence {
        Ok(planar)
    } else if linear.confidence >= DOMINANCE * planar.confidence {
        Ok(linear)
    } else {
        Err(MotionRejection::Disagree)
    }
}

/// Estimates the content motion of a region between two frames with the combined estimator and
/// the estimator of the current thread. See [`MotionEstimator::estimate`].
pub fn estimate_motion<'a, 'b>(
    prev: impl Into<ImageView<'a>>,
    next: impl Into<ImageView<'b>>,
    roi: Option<Rect<Physical>>,
    config: &MotionConfig,
) -> Result<Motion, MotionRejection> {
    let (prev, next) = (prev.into(), next.into());
    with_estimator(|e| e.estimate(prev, next, roi, config))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;
    use crate::synth::{stripes, text_page, texture};

    fn m(dx: f64, dy: f64, confidence: f64) -> Motion {
        Motion { dx, dy, confidence }
    }

    #[test]
    fn reconcile_rules() {
        let r = reconcile(m(0.2, -10.0, 0.5), m(0.0, -10.4, 0.8), Axis::Vertical).unwrap();
        assert_eq!((r.dx, r.dy, r.confidence), (0.2, -10.0, 0.8));
        let r = reconcile(m(0.0, -10.0, 0.9), m(0.0, 12.0, 0.3), Axis::Vertical).unwrap();
        assert_eq!(r.dy, -10.0);
        let r = reconcile(m(0.0, -10.0, 0.3), m(0.0, 12.0, 0.9), Axis::Vertical).unwrap();
        assert_eq!(r.dy, 12.0);
        assert_eq!(
            reconcile(m(0.0, -10.0, 0.6), m(0.0, 12.0, 0.7), Axis::Vertical),
            Err(MotionRejection::Disagree)
        );
    }

    #[test]
    fn text_scroll_with_axis() {
        let config = MotionConfig {
            axis: Some(Axis::Vertical),
            max_shift: 120,
            ..MotionConfig::default()
        };
        let a = text_page(900, 700, 0, 0, 21);
        for dy in [-88, -40, -3, 15, 66] {
            let b = text_page(900, 700, 0, dy, 21);
            let r = estimate_motion(&a, &b, None, &config).unwrap();
            assert!(
                (r.dy - dy as f64).abs() < 0.3 && r.dx.abs() < 0.3,
                "{dy} -> {r:?}"
            );
        }
    }

    #[test]
    fn planar_without_axis() {
        let a = texture(500, 400, 0.0, 0.0, 2);
        let b = texture(500, 400, -8.5, 12.25, 2);
        let r = estimate_motion(&a, &b, None, &MotionConfig::default()).unwrap();
        assert!(
            (r.dx + 8.5).abs() < 0.25 && (r.dy - 12.25).abs() < 0.25,
            "{r:?}"
        );
    }

    #[test]
    fn rejects_flat_and_repetitive() {
        let config = MotionConfig {
            axis: Some(Axis::Vertical),
            ..MotionConfig::default()
        };
        let flat = Image::filled(400, 300, [30, 30, 30, 255]);
        assert_eq!(
            estimate_motion(&flat, &flat, None, &config),
            Err(MotionRejection::Flat)
        );
        let a = stripes(400, 300, 24, 0);
        let b = stripes(400, 300, 24, 8);
        assert!(estimate_motion(&a, &b, None, &config).is_err());
    }
}
