//! Phase correlation of two grayscale patches with reusable FFT plans and buffers.
//!
//! Both patches lose their mean and get a Hann window. One complex FFT transforms both: the first
//! patch is the real part and the second patch the imaginary part. The cross-power spectrum, almost
//! whitened and low-pass weighted, transforms back into a surface whose peak sits at the
//! displacement.

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use super::peak::{Peak, find_peak};
use super::sizes::hann_into;

/// A grayscale patch inside a larger buffer. `data[0]` is the top-left pixel.
#[derive(Clone, Copy)]
pub(crate) struct Patch<'a> {
    /// Pixel values from the top-left pixel on.
    pub data: &'a [f32],
    /// Values between the starts of two rows.
    pub stride: usize,
}

impl Patch<'_> {
    fn at(&self, x: usize, y: usize) -> f32 {
        self.data[y * self.stride + x]
    }
}

/// FFT plans for one patch size.
struct Plans {
    w: usize,
    h: usize,
    fwd_w: Arc<dyn Fft<f32>>,
    fwd_h: Arc<dyn Fft<f32>>,
    inv_w: Arc<dyn Fft<f32>>,
    inv_h: Arc<dyn Fft<f32>>,
}

/// Phase correlation with cached plans, windows and buffers.
pub(crate) struct PhaseCorrelator {
    planner: FftPlanner<f32>,
    plans: Option<Plans>,
    win_x: Vec<f32>,
    win_y: Vec<f32>,
    buf: Vec<Complex<f32>>,
    tbuf: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    weights: Vec<f32>,
    surface: Vec<f32>,
}

impl std::fmt::Debug for PhaseCorrelator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PhaseCorrelator")
            .field("size", &self.plans.as_ref().map(|p| (p.w, p.h)))
            .finish_non_exhaustive()
    }
}

impl Default for PhaseCorrelator {
    fn default() -> Self {
        Self::new()
    }
}

impl PhaseCorrelator {
    /// Builds a correlator without plans.
    pub(crate) fn new() -> Self {
        Self {
            planner: FftPlanner::new(),
            plans: None,
            win_x: Vec::new(),
            win_y: Vec::new(),
            buf: Vec::new(),
            tbuf: Vec::new(),
            scratch: Vec::new(),
            weights: Vec::new(),
            surface: Vec::new(),
        }
    }

    fn prepare(&mut self, w: usize, h: usize) {
        if self.plans.as_ref().is_some_and(|p| p.w == w && p.h == h) {
            return;
        }
        let plans = Plans {
            w,
            h,
            fwd_w: self.planner.plan_fft_forward(w),
            fwd_h: self.planner.plan_fft_forward(h),
            inv_w: self.planner.plan_fft_inverse(w),
            inv_h: self.planner.plan_fft_inverse(h),
        };
        let scratch = [&plans.fwd_w, &plans.fwd_h, &plans.inv_w, &plans.inv_h]
            .iter()
            .map(|p| p.get_inplace_scratch_len())
            .max()
            .unwrap_or(0);
        self.scratch.resize(scratch, Complex::default());
        self.buf.resize(w * h, Complex::default());
        self.tbuf.resize(w * h, Complex::default());
        self.surface.resize(w * h, 0.0);
        hann_into(w, &mut self.win_x);
        hann_into(h, &mut self.win_y);
        low_pass_into(w, h, &mut self.weights);
        self.plans = Some(plans);
    }

    /// Correlates two `w` by `h` patches and returns the displacement of `b` against `a`, searched
    /// up to `max_dx` and `max_dy`. A perfect match peaks near one.
    pub(crate) fn correlate(
        &mut self,
        a: Patch<'_>,
        b: Patch<'_>,
        w: usize,
        h: usize,
        max_dx: usize,
        max_dy: usize,
    ) -> Peak {
        self.prepare(w, h);
        let (ma, mb) = (mean(a, w, h), mean(b, w, h));
        for y in 0..h {
            let wy = self.win_y[y];
            let row = &mut self.buf[y * w..(y + 1) * w];
            for (x, c) in row.iter_mut().enumerate() {
                let win = wy * self.win_x[x];
                *c = Complex::new((a.at(x, y) - ma) * win, (b.at(x, y) - mb) * win);
            }
        }
        let Some(plans) = self.plans.as_ref() else {
            unreachable!("prepare sets the plans");
        };
        plans
            .fwd_w
            .process_with_scratch(&mut self.buf, &mut self.scratch);
        transpose(&self.buf, &mut self.tbuf, w, h);
        plans
            .fwd_h
            .process_with_scratch(&mut self.tbuf, &mut self.scratch);
        cross_power(&self.tbuf, &mut self.buf, &self.weights, w, h);
        plans
            .inv_h
            .process_with_scratch(&mut self.buf, &mut self.scratch);
        transpose(&self.buf, &mut self.tbuf, h, w);
        plans
            .inv_w
            .process_with_scratch(&mut self.tbuf, &mut self.scratch);
        let norm = 1.0 / (w * h) as f32;
        for (s, c) in self.surface.iter_mut().zip(&self.tbuf) {
            *s = c.re * norm;
        }
        find_peak(&self.surface, w, h, max_dx, max_dy)
    }
}

fn mean(p: Patch<'_>, w: usize, h: usize) -> f32 {
    let mut sum = 0f64;
    for y in 0..h {
        sum += p.data[y * p.stride..y * p.stride + w]
            .iter()
            .map(|&v| v as f64)
            .sum::<f64>();
    }
    (sum / (w * h) as f64) as f32
}

/// Transposes a `w` by `h` row-major matrix into an `h` by `w` row-major matrix.
fn transpose(src: &[Complex<f32>], dst: &mut [Complex<f32>], w: usize, h: usize) {
    const BLOCK: usize = 16;
    for y0 in (0..h).step_by(BLOCK) {
        for x0 in (0..w).step_by(BLOCK) {
            for y in y0..(y0 + BLOCK).min(h) {
                for x in x0..(x0 + BLOCK).min(w) {
                    dst[x * h + y] = src[y * w + x];
                }
            }
        }
    }
}

/// Splits the joint spectrum `Z = A + iB` of two real patches and writes the normalized
/// cross-power spectrum `B conj(A) / |B conj(A)|`. Both buffers use the transposed layout: index
/// `kx * h + ky`.
/// Partial whitening: the cross-power magnitude gains this fraction of its mean before division.
/// Bins with little energy then weigh less than bins with structure.
const SOFT_WHITENING: f32 = 0.05;
/// Standard deviation of the Gaussian low-pass weight in cycles per pixel. The weight shapes the
/// correlation peak for the parabolic subpixel fit and damps noise.
const LOW_PASS_SIGMA: f32 = 0.15;

/// Writes the Gaussian low-pass weights in the transposed spectrum layout, scaled to a mean of
/// one so a perfect match keeps a peak near one.
fn low_pass_into(w: usize, h: usize, out: &mut Vec<f32>) {
    out.clear();
    let inv2s2 = 1.0 / (2.0 * LOW_PASS_SIGMA * LOW_PASS_SIGMA);
    for kx in 0..w {
        let fx = kx.min(w - kx) as f32 / w as f32;
        for ky in 0..h {
            let fy = ky.min(h - ky) as f32 / h as f32;
            out.push((-(fx * fx + fy * fy) * inv2s2).exp());
        }
    }
    let mean = out.iter().map(|&g| g as f64).sum::<f64>() / out.len() as f64;
    let scale = (1.0 / mean) as f32;
    out.iter_mut().for_each(|g| *g *= scale);
}

/// Splits the joint spectrum `Z = A + iB` of two real patches and writes the weighted, partially
/// whitened cross-power spectrum `B conj(A)`. Both buffers use the transposed layout: index
/// `kx * h + ky`.
fn cross_power(z: &[Complex<f32>], out: &mut [Complex<f32>], weights: &[f32], w: usize, h: usize) {
    let mut total = 0f64;
    for kx in 0..w {
        let nx = (w - kx) % w;
        for ky in 0..h {
            let ny = (h - ky) % h;
            let zk = z[kx * h + ky];
            let zn = z[nx * h + ny].conj();
            let a = (zk + zn) * 0.5;
            let d = (zk - zn) * 0.5;
            // (zk - conj(z[-k])) / 2i
            let b = Complex::new(d.im, -d.re);
            let p = b * a.conj();
            total += p.norm() as f64;
            out[kx * h + ky] = p;
        }
    }
    let mean = (total / (w * h) as f64) as f32;
    let eps = (SOFT_WHITENING * mean).max(1e-30);
    for (p, &g) in out.iter_mut().zip(weights) {
        let m = p.norm();
        *p *= g / (m + eps);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn patch_data(w: usize, h: usize, dx: f64, dy: f64) -> Vec<f32> {
        let img = crate::synth::texture(w as u32, h as u32, dx, dy, 77);
        img.view().to_gray_f32().data
    }

    #[test]
    fn recovers_integer_shift() {
        let (w, h) = (64, 48);
        let a = patch_data(w, h, 0.0, 0.0);
        let b = patch_data(w, h, 5.0, -3.0);
        let mut pc = PhaseCorrelator::new();
        let p = pc.correlate(
            Patch {
                data: &a,
                stride: w,
            },
            Patch {
                data: &b,
                stride: w,
            },
            w,
            h,
            20,
            20,
        );
        assert!((p.dx - 5.0).abs() < 0.2, "{p:?}");
        assert!((p.dy + 3.0).abs() < 0.2, "{p:?}");
        assert!(p.value > 0.3 && p.second < p.value * 0.5, "{p:?}");
    }

    #[test]
    fn transpose_round_trip() {
        let src: Vec<Complex<f32>> = (0..12).map(|i| Complex::new(i as f32, 0.0)).collect();
        let mut t = vec![Complex::default(); 12];
        let mut back = vec![Complex::default(); 12];
        transpose(&src, &mut t, 4, 3);
        assert_eq!(t[1].re, 4.0);
        transpose(&t, &mut back, 3, 4);
        assert_eq!(back, src);
    }
}
