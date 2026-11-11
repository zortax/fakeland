//! 2D translation by phase correlation: a coarse pass on the downscaled luma of the region, then a
//! refinement pass on a full-resolution patch.

use fakeland_types::{Physical, Rect};

use super::confidence::judge_peak;
use super::estimator::with_estimator;
use super::phase::Patch;
use super::sizes::smooth_floor;
use super::{Motion, MotionConfig, MotionEstimator, MotionRejection};
use crate::image::{ImageView, box_luma_into};

/// The smallest side of a correlated patch.
pub(crate) const MIN_SIDE: usize = 16;

/// The coarse grid of one estimate: the downscale factor and the correlated window inside the
/// downscaled luma.
#[derive(Clone, Copy, Debug)]
pub(crate) struct CoarseGrid {
    /// Source pixels per downscaled pixel.
    pub factor: usize,
    /// Width of the downscaled luma.
    pub stride: usize,
}

impl MotionEstimator {
    /// Estimates the 2D translation of the content of a region between two frames. `None`
    /// selects the whole frame; the region is clipped to the frames.
    pub fn translation<'a, 'b>(
        &mut self,
        prev: impl Into<ImageView<'a>>,
        next: impl Into<ImageView<'b>>,
        roi: Option<Rect<Physical>>,
        config: &MotionConfig,
    ) -> Result<Motion, MotionRejection> {
        let (va, vb) = regions(prev.into(), next.into(), roi)?;
        let (w, h) = (va.width() as usize, va.height() as usize);
        if w < MIN_SIDE || h < MIN_SIDE {
            return Err(MotionRejection::TooSmall);
        }
        let factor = w
            .max(h)
            .div_ceil(config.coarse_dim.max(MIN_SIDE as u32) as usize)
            .min(w / MIN_SIDE)
            .min(h / MIN_SIDE)
            .max(1);
        let (dw, dh) = box_luma_into(&va, factor as u32, &mut self.gray_a, &mut self.acc);
        box_luma_into(&vb, factor as u32, &mut self.gray_b, &mut self.acc);
        let (dw, dh) = (dw as usize, dh as usize);
        let (fw, fh) = (smooth_floor(dw), smooth_floor(dh));
        let start = (dh - fh) / 2 * dw + (dw - fw) / 2;
        let a = Patch {
            data: &self.gray_a[start..],
            stride: dw,
        };
        let b = Patch {
            data: &self.gray_b[start..],
            stride: dw,
        };
        if std_dev(a, fw, fh) < config.flat_std || std_dev(b, fw, fh) < config.flat_std {
            return Err(MotionRejection::Flat);
        }
        let reach = (config.max_shift as usize).div_ceil(factor);
        let peak = self.coarse.correlate(a, b, fw, fh, reach, reach);
        let confidence = judge_peak(&peak, config)?;
        let coarse = Motion {
            dx: peak.dx * factor as f64,
            dy: peak.dy * factor as f64,
            confidence,
        };
        if factor == 1 || config.refine_dim < MIN_SIDE as u32 {
            return Ok(coarse);
        }
        let grid = CoarseGrid { factor, stride: dw };
        Ok(self
            .refine(&va, &vb, coarse, grid, config)
            .unwrap_or(coarse))
    }
}

/// Estimates the 2D translation of the content of a region between two frames by phase
/// correlation, searched up to `max_shift` pixels per axis. Returns `None` for flat, ambiguous or
/// unreliable content. The estimate uses default settings and the estimator of the current thread.
pub fn estimate_translation<'a, 'b>(
    prev: impl Into<ImageView<'a>>,
    next: impl Into<ImageView<'b>>,
    roi: Option<Rect<Physical>>,
    max_shift: u32,
) -> Option<Motion> {
    let (prev, next) = (prev.into(), next.into());
    let config = MotionConfig::with_max_shift(max_shift);
    with_estimator(|e| e.translation(prev, next, roi, &config)).ok()
}

/// Returns the regions of both frames, or the reason they do not exist.
pub(crate) fn regions<'a, 'b>(
    prev: ImageView<'a>,
    next: ImageView<'b>,
    roi: Option<Rect<Physical>>,
) -> Result<(ImageView<'a>, ImageView<'b>), MotionRejection> {
    if !prev.same_size(&next) {
        return Err(MotionRejection::SizeMismatch);
    }
    let r = prev.clip(roi).ok_or(MotionRejection::EmptyRegion)?;
    match (prev.region(r), next.region(r)) {
        (Some(a), Some(b)) => Ok((a, b)),
        _ => Err(MotionRejection::EmptyRegion),
    }
}

/// Returns the standard deviation of a `w` by `h` patch.
pub(crate) fn std_dev(p: Patch<'_>, w: usize, h: usize) -> f64 {
    let (mut sum, mut sq) = (0f64, 0f64);
    for y in 0..h {
        for &v in &p.data[y * p.stride..y * p.stride + w] {
            sum += v as f64;
            sq += v as f64 * v as f64;
        }
    }
    let n = (w * h) as f64;
    let mean = sum / n;
    (sq / n - mean * mean).max(0.0).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;
    use crate::synth::{add_noise, dots, stripes, texture};

    fn check(w: u32, h: u32, dx: f64, dy: f64, tol: f64, noise: i32) -> Motion {
        let mut a = texture(w, h, 0.0, 0.0, 11);
        let mut b = texture(w, h, dx, dy, 11);
        if noise > 0 {
            add_noise(&mut a, noise, 1);
            add_noise(&mut b, noise, 2);
        }
        let m = estimate_translation(&a, &b, None, 64)
            .unwrap_or_else(|| panic!("no motion for ({dx}, {dy}) at {w}x{h}"));
        assert!(
            (m.dx - dx).abs() <= tol && (m.dy - dy).abs() <= tol,
            "({dx}, {dy}) at {w}x{h} -> {m:?}"
        );
        m
    }

    #[test]
    fn integer_shifts_small_region() {
        for (dx, dy) in [
            (0.0, 0.0),
            (7.0, 0.0),
            (0.0, -9.0),
            (-12.0, 5.0),
            (20.0, 17.0),
        ] {
            check(200, 160, dx, dy, 0.1, 0);
        }
    }

    #[test]
    fn integer_shifts_large_region_use_refinement() {
        for (dx, dy) in [(0.0, -37.0), (23.0, 0.0), (-5.0, 3.0), (41.0, -29.0)] {
            check(960, 600, dx, dy, 0.1, 0);
        }
    }

    #[test]
    fn subpixel_shifts() {
        for (dx, dy) in [(0.5, 0.0), (0.0, -2.25), (3.7, 1.3), (-10.4, 6.6)] {
            check(240, 200, dx, dy, 0.15, 0);
            check(720, 480, dx, dy, 0.15, 0);
        }
    }

    #[test]
    fn noisy_shifts() {
        for (dx, dy) in [(4.0, -6.0), (-15.5, 2.5)] {
            check(400, 300, dx, dy, 0.2, 12);
        }
    }

    #[test]
    fn flat_and_noise_rejected() {
        let a = Image::filled(300, 200, [120, 120, 120, 255]);
        let config = MotionConfig::default();
        let mut e = MotionEstimator::new();
        assert_eq!(
            e.translation(&a, &a, None, &config),
            Err(MotionRejection::Flat)
        );
        let (mut na, mut nb) = (a.clone(), a.clone());
        add_noise(&mut na, 20, 5);
        add_noise(&mut nb, 20, 6);
        let r = e.translation(&na, &nb, None, &config);
        assert!(
            matches!(
                r,
                Err(MotionRejection::LowConfidence { .. } | MotionRejection::Ambiguous { .. })
            ),
            "{r:?}"
        );
    }

    #[test]
    fn periodic_patterns_are_ambiguous() {
        let a = dots(256, 256, 16, 0, 0);
        let b = dots(256, 256, 16, 3, 5);
        let r = MotionEstimator::new().translation(&a, &b, None, &MotionConfig::default());
        assert!(
            matches!(
                r,
                Err(MotionRejection::Ambiguous { .. } | MotionRejection::LowConfidence { .. })
            ),
            "{r:?}"
        );
        // A search range below half the period leaves one candidate.
        let m = MotionEstimator::new()
            .translation(&a, &b, None, &MotionConfig::with_max_shift(7))
            .unwrap();
        assert!(
            (m.dx - 3.0).abs() < 0.3 && (m.dy - 5.0).abs() < 0.3,
            "{m:?}"
        );
        // Stripes carry no horizontal structure, so every horizontal displacement fits.
        let a = stripes(256, 256, 16, 0);
        let b = stripes(256, 256, 16, 5);
        let r = MotionEstimator::new().translation(&a, &b, None, &MotionConfig::with_max_shift(7));
        assert!(r.is_err(), "{r:?}");
    }

    #[test]
    fn roi_and_errors() {
        let a = texture(300, 200, 0.0, 0.0, 3);
        let mut b = texture(300, 200, 6.0, -4.0, 3);
        // A region that excludes a changed corner sees the clean shift.
        b.fill_rect(Rect::new(250, 0, 50, 50), [0, 0, 0, 255]);
        let m = estimate_translation(&a, &b, Some(Rect::new(0, 60, 240, 140)), 32).unwrap();
        assert!(
            (m.dx - 6.0).abs() < 0.2 && (m.dy + 4.0).abs() < 0.2,
            "{m:?}"
        );
        let mut e = MotionEstimator::new();
        let c = MotionConfig::default();
        let small = Image::new(10, 10);
        assert_eq!(
            e.translation(&a, &small, None, &c),
            Err(MotionRejection::SizeMismatch)
        );
        assert_eq!(
            e.translation(&a, &b, Some(Rect::new(400, 0, 5, 5)), &c),
            Err(MotionRejection::EmptyRegion)
        );
        assert_eq!(
            e.translation(&a, &b, Some(Rect::new(0, 0, 100, 10)), &c),
            Err(MotionRejection::TooSmall)
        );
    }
}
