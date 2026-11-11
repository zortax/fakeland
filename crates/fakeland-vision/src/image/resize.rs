//! Downscaling by an integer box filter and by area averaging to a maximum dimension.

use super::view::BPP;
use super::{Image, ImageView};

/// Source pixel weights of each output pixel along one axis.
struct AxisWeights {
    /// First source index of each output pixel.
    first: Vec<usize>,
    /// Offset of each output pixel's weights in `weights`, plus a final end offset.
    offsets: Vec<usize>,
    /// Weights that sum to one per output pixel.
    weights: Vec<f32>,
}

impl AxisWeights {
    /// Builds the area weights that map `src` pixels onto `dst` pixels.
    fn new(src: usize, dst: usize) -> Self {
        let scale = src as f64 / dst as f64;
        let mut first = Vec::with_capacity(dst);
        let mut offsets = Vec::with_capacity(dst + 1);
        let mut weights = Vec::new();
        for i in 0..dst {
            let lo = i as f64 * scale;
            let hi = ((i + 1) as f64 * scale).min(src as f64);
            let start = lo.floor() as usize;
            let end = (hi.ceil() as usize).min(src);
            first.push(start);
            offsets.push(weights.len());
            for j in start..end {
                let overlap = (hi.min(j as f64 + 1.0) - lo.max(j as f64)).max(0.0);
                weights.push((overlap / (hi - lo)) as f32);
            }
        }
        offsets.push(weights.len());
        Self {
            first,
            offsets,
            weights,
        }
    }

    /// Returns the first source index and the weights of an output pixel.
    fn taps(&self, i: usize) -> (usize, &[f32]) {
        (
            self.first[i],
            &self.weights[self.offsets[i]..self.offsets[i + 1]],
        )
    }
}

impl ImageView<'_> {
    /// Downscales by an integer box filter. The output has `ceil(width / factor)` by
    /// `ceil(height / factor)` pixels. Output pixel `(x, y)` is the mean of the source block that
    /// starts at `(x * factor, y * factor)`. A factor of zero counts as one.
    pub fn downscale(&self, factor: u32) -> Image {
        let f = factor.max(1) as usize;
        if f == 1 {
            return self.to_image();
        }
        let (w, h) = (self.width() as usize, self.height() as usize);
        let (ow, oh) = (w.div_ceil(f), h.div_ceil(f));
        let mut out = Vec::with_capacity(ow * oh * BPP);
        let mut acc = vec![0u32; ow * BPP];
        for by in 0..oh {
            acc.fill(0);
            let y0 = by * f;
            let y1 = (y0 + f).min(h);
            for sy in y0..y1 {
                let row = self.row(sy as u32);
                for (a, block) in acc.chunks_exact_mut(BPP).zip(row.chunks(f * BPP)) {
                    for px in block.chunks_exact(BPP) {
                        a[0] += px[0] as u32;
                        a[1] += px[1] as u32;
                        a[2] += px[2] as u32;
                        a[3] += px[3] as u32;
                    }
                }
            }
            for (bx, a) in acc.chunks_exact(BPP).enumerate() {
                let bw = (f).min(w - bx * f);
                let count = (bw * (y1 - y0)) as u32;
                let half = count / 2;
                out.extend(a.iter().map(|&s| ((s + half) / count) as u8));
            }
        }
        Image::from_packed(ow as u32, oh as u32, out)
    }

    /// Downscales by area averaging so that neither dimension exceeds `max_dim`. Returns the image
    /// and the scale factor: source pixels per output pixel along the longer axis. Output pixel
    /// `(x, y)` maps to source position `(x * scale, y * scale)`. An image that already fits is
    /// copied with a scale of one.
    pub fn downscale_to_max_dim(&self, max_dim: u32) -> (Image, f64) {
        let (w, h) = (self.width() as usize, self.height() as usize);
        let longest = w.max(h);
        let max_dim = max_dim.max(1) as usize;
        if longest <= max_dim {
            return (self.to_image(), 1.0);
        }
        let scale = longest as f64 / max_dim as f64;
        let ow = ((w as f64 / scale).round() as usize).clamp(1, max_dim);
        let oh = ((h as f64 / scale).round() as usize).clamp(1, max_dim);
        if w % ow == 0 && h % oh == 0 && w / ow == h / oh {
            return (self.downscale((w / ow) as u32), scale);
        }
        (self.resample_area(ow, oh), scale)
    }

    /// Resamples to `ow` by `oh` pixels by area averaging. Each source row is filtered
    /// horizontally once into a ring of cached rows; output rows blend the cached rows.
    fn resample_area(&self, ow: usize, oh: usize) -> Image {
        let xw = AxisWeights::new(self.width() as usize, ow);
        let yw = AxisWeights::new(self.height() as usize, oh);
        let slots = (0..oh).map(|i| yw.taps(i).1.len()).max().unwrap_or(1);
        let row_len = ow * BPP;
        let mut cache = vec![0f32; slots * row_len];
        let mut cached = vec![usize::MAX; slots];
        let mut out = Vec::with_capacity(ow * oh * BPP);
        let mut acc = vec![0f32; row_len];
        for oy in 0..oh {
            acc.fill(0.0);
            let (y0, ywts) = yw.taps(oy);
            for (k, &wy) in ywts.iter().enumerate() {
                let sy = y0 + k;
                let slot = sy % slots;
                let hrow = &mut cache[slot * row_len..(slot + 1) * row_len];
                if cached[slot] != sy {
                    filter_row(self.row(sy as u32), &xw, hrow);
                    cached[slot] = sy;
                }
                for (a, &h) in acc.iter_mut().zip(hrow.iter()) {
                    *a += wy * h;
                }
            }
            // Float to integer casts saturate, so the sum rounds and clamps in one step.
            out.extend(acc.iter().map(|&v| (v + 0.5) as u8));
        }
        Image::from_packed(ow as u32, oh as u32, out)
    }
}

/// Filters one RGBA row horizontally with area weights.
fn filter_row(row: &[u8], xw: &AxisWeights, out: &mut [f32]) {
    for (ox, dst) in out.chunks_exact_mut(BPP).enumerate() {
        let (x0, taps) = xw.taps(ox);
        let src = &row[x0 * BPP..(x0 + taps.len()) * BPP];
        let mut px = [0f32; BPP];
        for (s, &wx) in src.chunks_exact(BPP).zip(taps) {
            px[0] += wx * s[0] as f32;
            px[1] += wx * s[1] as f32;
            px[2] += wx * s[2] as f32;
            px[3] += wx * s[3] as f32;
        }
        dst.copy_from_slice(&px);
    }
}

impl Image {
    /// Downscales by an integer box filter. See [`ImageView::downscale`].
    pub fn downscale(&self, factor: u32) -> Image {
        self.view().downscale(factor)
    }

    /// Downscales to fit a maximum dimension. See [`ImageView::downscale_to_max_dim`].
    pub fn downscale_to_max_dim(&self, max_dim: u32) -> (Image, f64) {
        self.view().downscale_to_max_dim(max_dim)
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::Rect;

    use super::*;

    #[test]
    fn box_filter_maps_blocks() {
        let mut img = Image::filled(10, 7, [0, 0, 0, 255]);
        img.fill_rect(Rect::new(4, 2, 2, 2), [200, 100, 40, 255]);
        img.fill_rect(Rect::new(8, 6, 2, 1), [90, 90, 90, 255]);
        let d = img.downscale(2);
        assert_eq!((d.width(), d.height()), (5, 4));
        assert_eq!(d.pixel(2, 1), Some([200, 100, 40, 255]));
        assert_eq!(d.pixel(1, 1), Some([0, 0, 0, 255]));
        // The bottom-right block holds two source pixels.
        assert_eq!(d.pixel(4, 3), Some([90, 90, 90, 255]));
    }

    #[test]
    fn box_filter_averages() {
        let mut img = Image::filled(4, 4, [0, 0, 0, 0]);
        img.fill_rect(Rect::new(0, 0, 1, 4), [255, 255, 255, 255]);
        let d = img.downscale(4);
        assert_eq!(d.pixel(0, 0), Some([64, 64, 64, 64]));
    }

    #[test]
    fn max_dim_integer_and_fractional() {
        let img = Image::filled(1920, 1080, [10, 20, 30, 255]);
        let (d, s) = img.downscale_to_max_dim(960);
        assert_eq!((d.width(), d.height(), s), (960, 540, 2.0));
        let (d, s) = img.downscale_to_max_dim(1568);
        assert_eq!((d.width(), d.height()), (1568, 882));
        assert!((s - 1920.0 / 1568.0).abs() < 1e-12);
        assert_eq!(d.pixel(1000, 500), Some([10, 20, 30, 255]));
        let (d, s) = img.downscale_to_max_dim(4000);
        assert_eq!((d.width(), s), (1920, 1.0));
    }

    #[test]
    fn fractional_mapping_keeps_features_in_place() {
        let mut img = Image::filled(1000, 600, [0, 0, 0, 255]);
        img.fill_rect(Rect::new(700, 300, 30, 30), [255, 255, 255, 255]);
        let (d, s) = img.downscale_to_max_dim(300);
        let cx = 715.0 / s;
        let cy = 315.0 / s;
        assert_eq!(d.pixel(cx as u32, cy as u32).map(|p| p[0]), Some(255));
        assert_eq!(d.pixel(cx as u32 - 8, cy as u32).map(|p| p[0]), Some(0));
    }

    #[test]
    fn area_weights_sum_to_one() {
        let w = AxisWeights::new(1920, 1568);
        for i in 0..1568 {
            let (_, taps) = w.taps(i);
            let sum: f32 = taps.iter().sum();
            assert!((sum - 1.0).abs() < 1e-4, "output {i} sums to {sum}");
        }
    }
}
