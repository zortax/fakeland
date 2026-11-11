//! Synthetic test images: smooth textures with exact subpixel shifts, text-like pages, stripes and
//! noise.

use fakeland_types::Rect;

use crate::image::Image;

/// A small deterministic random generator.
pub(crate) struct Rng(u64);

impl Rng {
    /// Builds a generator from a seed.
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }

    /// Returns the next 64 random bits.
    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Returns a value in `0..1`.
    pub(crate) fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Returns a value in `lo..hi`.
    pub(crate) fn range(&mut self, lo: f64, hi: f64) -> f64 {
        lo + (hi - lo) * self.unit()
    }

    /// Returns an integer in `lo..hi`.
    pub(crate) fn int(&mut self, lo: i32, hi: i32) -> i32 {
        lo + (self.next_u64() % (hi - lo) as u64) as i32
    }
}

/// A lattice of random values with bicubic interpolation: a continuous random field.
struct ValueNoise {
    size: usize,
    values: Vec<f64>,
}

impl ValueNoise {
    fn new(size: usize, rng: &mut Rng) -> Self {
        Self {
            size,
            values: (0..size * size).map(|_| rng.range(-1.0, 1.0)).collect(),
        }
    }

    fn lattice(&self, x: i64, y: i64) -> f64 {
        let n = self.size as i64;
        self.values[(y.rem_euclid(n) * n + x.rem_euclid(n)) as usize]
    }

    fn sample(&self, x: f64, y: f64) -> f64 {
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (ix, iy) = (x0 as i64, y0 as i64);
        let mut col = [0.0; 4];
        for (j, c) in col.iter_mut().enumerate() {
            let yy = iy + j as i64 - 1;
            *c = cubic(
                [
                    self.lattice(ix - 1, yy),
                    self.lattice(ix, yy),
                    self.lattice(ix + 1, yy),
                    self.lattice(ix + 2, yy),
                ],
                fx,
            );
        }
        cubic(col, fy)
    }
}

/// Catmull-Rom interpolation between `p[1]` and `p[2]`.
fn cubic(p: [f64; 4], t: f64) -> f64 {
    p[1] + 0.5
        * t
        * (p[2] - p[0]
            + t * (2.0 * p[0] - 5.0 * p[1] + 4.0 * p[2] - p[3]
                + t * (3.0 * (p[1] - p[2]) + p[3] - p[0])))
}

/// Renders a broadband random texture sampled at `(x - dx, y - dy)`. Content moves by
/// `(dx, dy)`. Octaves of value noise span cell sizes from 3 to 48 pixels.
pub(crate) fn texture(w: u32, h: u32, dx: f64, dy: f64, seed: u64) -> Image {
    let mut rng = Rng::new(seed);
    let octaves: Vec<(f64, f64, ValueNoise)> = [3.0f64, 6.0, 12.0, 24.0, 48.0]
        .into_iter()
        .map(|cell| (cell, cell.powf(0.5), ValueNoise::new(97, &mut rng)))
        .collect();
    let norm: f64 = octaves.iter().map(|o| o.1).sum::<f64>() * 0.45;
    let mut img = Image::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let (sx, sy) = (x as f64 - dx, y as f64 - dy);
            let v: f64 = octaves
                .iter()
                .map(|(cell, amp, noise)| amp * noise.sample(sx / cell, sy / cell))
                .sum();
            let g = (128.0 + 127.0 * (v / norm).clamp(-1.0, 1.0)).round() as u8;
            img.set_pixel(x, y, [g, g / 2 + 60, 255 - g, 255]);
        }
    }
    img
}

/// Renders a lattice of dark squares with a fixed period on white, moved by whole pixels.
pub(crate) fn dots(w: u32, h: u32, period: i32, dx: i32, dy: i32) -> Image {
    let mut img = Image::filled(w, h, [255, 255, 255, 255]);
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if (x - dx).rem_euclid(period) < period / 4 && (y - dy).rem_euclid(period) < period / 4
            {
                img.set_pixel(x as u32, y as u32, [0, 0, 0, 255]);
            }
        }
    }
    img
}

/// Renders a page of text-like glyph boxes on white, viewed with the content moved by
/// `(dx, dy)` whole pixels.
pub(crate) fn text_page(w: u32, h: u32, dx: i32, dy: i32, seed: u64) -> Image {
    let mut img = Image::filled(w, h, [255, 255, 255, 255]);
    let mut rng = Rng::new(seed);
    let line_height = 22;
    let margin = 24;
    // The page extends past the view so that moved content stays filled.
    let lines = (h as i32 + 2 * 400) / line_height;
    for line in 0..lines {
        let top = line * line_height - 400 + 6;
        let line_end = margin + rng.int(w as i32 / 3, w as i32 - margin);
        let mut x = margin - 400;
        while x < line_end + 400 {
            let word = rng.int(2, 9);
            for _ in 0..word {
                let gw = rng.int(4, 9);
                let gh = rng.int(8, 13);
                let ascent = rng.int(0, 3);
                let shade = rng.int(0, 70) as u8;
                if x >= margin && x + gw <= line_end {
                    img.fill_rect(
                        Rect::new(x + dx, top + ascent + (12 - gh) + dy, gw, gh),
                        [shade, shade, shade, 255],
                    );
                }
                x += gw + 2;
            }
            x += rng.int(6, 12);
        }
    }
    img
}

/// Renders horizontal stripes with a fixed period, moved down by `dy` whole pixels.
pub(crate) fn stripes(w: u32, h: u32, period: i32, dy: i32) -> Image {
    let mut img = Image::filled(w, h, [255, 255, 255, 255]);
    for y in 0..h as i32 {
        if (y - dy).rem_euclid(period) < period / 2 {
            img.fill_rect(Rect::new(0, y, w as i32, 1), [0, 0, 0, 255]);
        }
    }
    img
}

/// Adds uniform noise of amplitude `amp` to the color channels.
pub(crate) fn add_noise(img: &mut Image, amp: i32, seed: u64) {
    let mut rng = Rng::new(seed);
    for y in 0..img.height() {
        for px in img.row_mut(y).chunks_exact_mut(4) {
            for c in px.iter_mut().take(3) {
                *c = (*c as i32 + rng.int(-amp, amp + 1)).clamp(0, 255) as u8;
            }
        }
    }
}
