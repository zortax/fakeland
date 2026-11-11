//! Refinement of a coarse translation on a full-resolution patch. The patch sits where the coarse
//! luma has the most gradient energy, and its partner in the later frame sits at the rounded coarse
//! displacement.

use fakeland_types::Rect;

use super::confidence::judge_peak;
use super::phase::Patch;
use super::sizes::smooth_floor;
use super::translation::{CoarseGrid, std_dev};
use super::{Motion, MotionConfig, MotionEstimator};
use crate::image::{ImageView, box_luma_into};

/// The smallest side of a refinement patch.
const MIN_PATCH: usize = 32;
/// Candidate positions per axis for the refinement patch.
const CANDIDATES: usize = 4;

impl MotionEstimator {
    /// Refines a coarse estimate. Returns `None` when no patch fits or the patch result is
    /// unreliable.
    pub(crate) fn refine(
        &mut self,
        va: &ImageView<'_>,
        vb: &ImageView<'_>,
        coarse: Motion,
        grid: CoarseGrid,
        config: &MotionConfig,
    ) -> Option<Motion> {
        let (w, h) = (va.width() as i64, va.height() as i64);
        let (ix, iy) = (coarse.dx.round() as i64, coarse.dy.round() as i64);
        let side = config.refine_dim as i64;
        let pw = smooth_floor(side.min(w - ix.abs()).max(0) as usize);
        let ph = smooth_floor(side.min(h - iy.abs()).max(0) as usize);
        if pw < MIN_PATCH || ph < MIN_PATCH {
            return None;
        }
        let xs = (ix.min(0).abs(), w - pw as i64 - ix.max(0));
        let ys = (iy.min(0).abs(), h - ph as i64 - iy.max(0));
        let (x, y) = self.best_origin(xs, ys, pw, ph, grid);
        let ra = Rect::new(x as i32, y as i32, pw as i32, ph as i32);
        let rb = ra.translate(ix as i32, iy as i32);
        box_luma_into(&va.region(ra)?, 1, &mut self.patch_a, &mut self.acc);
        box_luma_into(&vb.region(rb)?, 1, &mut self.patch_b, &mut self.acc);
        let a = Patch {
            data: &self.patch_a,
            stride: pw,
        };
        let b = Patch {
            data: &self.patch_b,
            stride: pw,
        };
        if std_dev(a, pw, ph) < config.flat_std || std_dev(b, pw, ph) < config.flat_std {
            return None;
        }
        let reach = grid.factor + 2;
        let peak = self.fine.correlate(a, b, pw, ph, reach, reach);
        judge_peak(&peak, config).ok()?;
        let limit = grid.factor as f64 + 1.0;
        if peak.dx.abs() > limit || peak.dy.abs() > limit {
            return None;
        }
        Some(Motion {
            dx: ix as f64 + peak.dx,
            dy: iy as f64 + peak.dy,
            confidence: coarse.confidence,
        })
    }

    /// Picks the patch origin with the most gradient energy in the coarse luma among a grid of
    /// candidates in the allowed ranges.
    fn best_origin(
        &self,
        xs: (i64, i64),
        ys: (i64, i64),
        pw: usize,
        ph: usize,
        grid: CoarseGrid,
    ) -> (i64, i64) {
        let pick = |range: (i64, i64), i: usize| {
            range.0 + (range.1 - range.0) * i as i64 / (CANDIDATES as i64 - 1)
        };
        let mut best = (pick(xs, 1), pick(ys, 1), f64::NEG_INFINITY);
        for j in 0..CANDIDATES {
            for i in 0..CANDIDATES {
                let (x, y) = (pick(xs, i), pick(ys, j));
                let score = self.gradient_energy(x, y, pw, ph, grid);
                if score > best.2 {
                    best = (x, y, score);
                }
            }
        }
        (best.0, best.1)
    }

    /// Returns the summed absolute gradient of the coarse luma under a full-resolution rectangle.
    fn gradient_energy(&self, x: i64, y: i64, pw: usize, ph: usize, grid: CoarseGrid) -> f64 {
        let f = grid.factor;
        let rows = self.gray_a.len() / grid.stride;
        let (x0, y0) = (x as usize / f, y as usize / f);
        let x1 = ((x as usize + pw) / f).min(grid.stride - 1);
        let y1 = ((y as usize + ph) / f).min(rows.saturating_sub(1));
        let g = &self.gray_a;
        let mut sum = 0f64;
        for cy in y0..y1 {
            for cx in x0..x1 {
                let v = g[cy * grid.stride + cx];
                sum += ((g[cy * grid.stride + cx + 1] - v).abs()
                    + (g[(cy + 1) * grid.stride + cx] - v).abs()) as f64;
            }
        }
        sum
    }
}
