//! 1D scroll estimation along one axis: normalized cross-correlation of band projection profiles
//! over every candidate displacement, with parabolic subpixel refinement.

use fakeland_types::{Physical, Rect};

use super::confidence::judge_ncc;
use super::estimator::with_estimator;
use super::peak::parabolic;
use super::profile::{band_profiles, max_band_std};
use super::translation::{MIN_SIDE, regions};
use super::{Axis, Motion, MotionConfig, MotionEstimator, MotionRejection};
use crate::image::ImageView;

/// Pixels across the axis per profile band.
const BAND_SPAN: usize = 64;
/// The most profile bands.
const MAX_BANDS: usize = 8;
/// Candidates within this distance of the best belong to its peak.
const PEAK_RADIUS: usize = 2;

impl MotionEstimator {
    /// Estimates the displacement of the content of a region along one axis. The other component
    /// of the result is zero. `None` selects the whole frame; the region is clipped to the frames.
    pub fn scroll_1d<'a, 'b>(
        &mut self,
        prev: impl Into<ImageView<'a>>,
        next: impl Into<ImageView<'b>>,
        roi: Option<Rect<Physical>>,
        axis: Axis,
        config: &MotionConfig,
    ) -> Result<Motion, MotionRejection> {
        let (va, vb) = regions(prev.into(), next.into(), roi)?;
        let (along, across) = match axis {
            Axis::Vertical => (va.height() as usize, va.width() as usize),
            Axis::Horizontal => (va.width() as usize, va.height() as usize),
        };
        if along < 2 * MIN_SIDE || across < 2 {
            return Err(MotionRejection::TooSmall);
        }
        let bands = (across / BAND_SPAN).clamp(1, MAX_BANDS);
        let len = band_profiles(&va, axis, bands, &mut self.profile_a);
        band_profiles(&vb, axis, bands, &mut self.profile_b);
        if max_band_std(&self.profile_a, len) < config.flat_std
            || max_band_std(&self.profile_b, len) < config.flat_std
        {
            return Err(MotionRejection::Flat);
        }
        let min_overlap = (len / 3).max(MIN_SIDE);
        let reach = (config.max_shift as usize).min(len - min_overlap);
        self.fill_prefix(len, bands);
        self.scores.clear();
        for s in -(reach as i64)..=reach as i64 {
            let score = self.ncc(len, bands, s);
            self.scores.push(score);
        }
        let (best_i, best) =
            self.scores
                .iter()
                .copied()
                .enumerate()
                .fold(
                    (0, f64::NEG_INFINITY),
                    |acc, (i, v)| {
                        if v > acc.1 { (i, v) } else { acc }
                    },
                );
        let second = second_local_max(&self.scores, best_i);
        let confidence = judge_ncc(best, second, config)?;
        let at = |i: usize| self.scores.get(i).copied().unwrap_or(best);
        let frac = if best_i == 0 || best_i + 1 == self.scores.len() {
            0.0
        } else {
            parabolic(best, at(best_i - 1), at(best_i + 1))
        };
        let shift = best_i as f64 - reach as f64 + frac;
        let (dx, dy) = match axis {
            Axis::Vertical => (0.0, shift),
            Axis::Horizontal => (shift, 0.0),
        };
        Ok(Motion { dx, dy, confidence })
    }

    /// Fills prefix sums of values and squares of both profile sets.
    fn fill_prefix(&mut self, len: usize, bands: usize) {
        let stride = len + 1;
        self.prefix.clear();
        self.prefix.resize(4 * bands * stride, 0.0);
        for (set, profiles) in [&self.profile_a, &self.profile_b].into_iter().enumerate() {
            for b in 0..bands {
                let p = &profiles[b * len..(b + 1) * len];
                let base1 = (2 * set) * bands * stride + b * stride;
                let base2 = (2 * set + 1) * bands * stride + b * stride;
                let (mut s1, mut s2) = (0f64, 0f64);
                for (i, &v) in p.iter().enumerate() {
                    s1 += v as f64;
                    s2 += v as f64 * v as f64;
                    self.prefix[base1 + i + 1] = s1;
                    self.prefix[base2 + i + 1] = s2;
                }
            }
        }
    }

    /// Returns the normalized cross-correlation of the profiles for displacement `s`: the later
    /// profile at `i` pairs with the earlier profile at `i - s`.
    fn ncc(&self, len: usize, bands: usize, s: i64) -> f64 {
        let n = len - s.unsigned_abs() as usize;
        let i0 = s.max(0) as usize;
        let j0 = (-s).max(0) as usize;
        let stride = len + 1;
        let range = |set: usize, b: usize, start: usize| {
            let base = set * bands * stride + b * stride;
            self.prefix[base + start + n] - self.prefix[base + start]
        };
        let (mut cov, mut vp, mut vq) = (0f64, 0f64, 0f64);
        for b in 0..bands {
            let p = &self.profile_a[b * len + j0..b * len + j0 + n];
            let q = &self.profile_b[b * len + i0..b * len + i0 + n];
            let (sp, spp) = (range(0, b, j0), range(1, b, j0));
            let (sq, sqq) = (range(2, b, i0), range(3, b, i0));
            let nf = n as f64;
            cov += dot(p, q) - sp * sq / nf;
            vp += spp - sp * sp / nf;
            vq += sqq - sq * sq / nf;
        }
        let denom = (vp * vq).sqrt();
        if denom <= 1e-9 { 0.0 } else { cov / denom }
    }
}

/// Returns the dot product of two equal-length slices.
fn dot(a: &[f32], b: &[f32]) -> f64 {
    let mut lanes = [0f32; 8];
    let (ca, cb) = (a.chunks_exact(8), b.chunks_exact(8));
    let tail: f32 = ca
        .remainder()
        .iter()
        .zip(cb.remainder())
        .map(|(x, y)| x * y)
        .sum();
    for (x, y) in ca.zip(cb) {
        for k in 0..8 {
            lanes[k] += x[k] * y[k];
        }
    }
    lanes.iter().map(|&v| v as f64).sum::<f64>() + tail as f64
}

/// Returns the largest local maximum farther than the peak radius from `best`, or -1 when none
/// exists.
fn second_local_max(scores: &[f64], best: usize) -> f64 {
    let mut second = -1f64;
    for i in 0..scores.len() {
        if i.abs_diff(best) <= PEAK_RADIUS {
            continue;
        }
        let left = i == 0 || scores[i] >= scores[i - 1];
        let right = i + 1 == scores.len() || scores[i] >= scores[i + 1];
        if left && right {
            second = second.max(scores[i]);
        }
    }
    second
}

/// Estimates the displacement of the content of a region along one axis, searched up to
/// `max_shift` pixels. Returns `None` for flat, ambiguous or unreliable content. The estimate uses
/// default settings and the estimator of the current thread.
pub fn estimate_scroll_1d<'a, 'b>(
    prev: impl Into<ImageView<'a>>,
    next: impl Into<ImageView<'b>>,
    roi: Option<Rect<Physical>>,
    axis: Axis,
    max_shift: u32,
) -> Option<Motion> {
    let (prev, next) = (prev.into(), next.into());
    let config = MotionConfig::with_max_shift(max_shift);
    with_estimator(|e| e.scroll_1d(prev, next, roi, axis, &config)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;
    use crate::synth::{add_noise, stripes, text_page, texture};

    #[test]
    fn text_scrolls_vertically() {
        let a = text_page(640, 480, 0, 0, 4);
        for dy in [-60, -23, -1, 0, 7, 22, 44, 130] {
            let b = text_page(640, 480, 0, dy, 4);
            let m = estimate_scroll_1d(&a, &b, None, Axis::Vertical, 200)
                .unwrap_or_else(|| panic!("no scroll for {dy}"));
            assert!((m.dy - dy as f64).abs() < 0.3, "{dy} -> {m:?}");
            assert_eq!(m.dx, 0.0);
        }
    }

    #[test]
    fn noisy_text_scrolls() {
        let mut a = text_page(800, 600, 0, 0, 9);
        let mut b = text_page(800, 600, 0, -35, 9);
        add_noise(&mut a, 25, 1);
        add_noise(&mut b, 25, 2);
        let m = estimate_scroll_1d(&a, &b, None, Axis::Vertical, 100).unwrap();
        assert!((m.dy + 35.0).abs() < 0.3, "{m:?}");
    }

    #[test]
    fn horizontal_scroll_and_subpixel() {
        let a = texture(400, 120, 0.0, 0.0, 8);
        let b = texture(400, 120, -17.4, 0.0, 8);
        let m = estimate_scroll_1d(&a, &b, None, Axis::Horizontal, 64).unwrap();
        assert!((m.dx + 17.4).abs() < 0.3, "{m:?}");
        let b = texture(400, 120, 0.0, 0.0, 8);
        let m = estimate_scroll_1d(&a, &b, None, Axis::Horizontal, 64).unwrap();
        assert!(m.dx.abs() < 0.05 && m.confidence > 0.9, "{m:?}");
    }

    #[test]
    fn flat_and_periodic_rejected() {
        let flat = Image::filled(300, 300, [250, 250, 250, 255]);
        let mut e = MotionEstimator::new();
        let c = MotionConfig::default();
        assert_eq!(
            e.scroll_1d(&flat, &flat, None, Axis::Vertical, &c),
            Err(MotionRejection::Flat)
        );
        let a = stripes(300, 300, 20, 0);
        let b = stripes(300, 300, 20, 6);
        let r = e.scroll_1d(&a, &b, None, Axis::Vertical, &c);
        assert!(matches!(r, Err(MotionRejection::Ambiguous { .. })), "{r:?}");
        let m = estimate_scroll_1d(&a, &b, None, Axis::Vertical, 9).unwrap();
        assert!((m.dy - 6.0).abs() < 0.3, "{m:?}");
    }

    #[test]
    fn helpers() {
        assert_eq!(second_local_max(&[0.1, 0.5, 0.2, 0.9, 1.0, 0.95], 4), 0.5);
        assert_eq!(second_local_max(&[1.0, 0.5], 0), -1.0);
        let a: Vec<f32> = (0..21).map(|i| i as f32).collect();
        assert_eq!(dot(&a, &a), (0..21).map(|i| (i * i) as f64).sum::<f64>());
    }
}
