//! Grayscale images and the luma conversion of RGBA8 pixels.
//!
//! Luma uses the BT.601 weights in 8-bit fixed point: `(77 R + 150 G + 29 B) / 256`.

use super::ImageView;
use super::view::BPP;

/// Returns the fixed-point luma sum of one pixel, 256 times the luma.
#[inline]
pub(crate) fn luma_sum(px: &[u8]) -> u32 {
    77 * px[0] as u32 + 150 * px[1] as u32 + 29 * px[2] as u32
}

/// Returns the luma of an RGB value.
#[inline]
pub fn luma(r: u8, g: u8, b: u8) -> u8 {
    ((77 * r as u32 + 150 * g as u32 + 29 * b as u32 + 128) >> 8) as u8
}

/// An owned 8-bit grayscale image with packed rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrayImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// One byte per pixel, `width` bytes per row.
    pub data: Vec<u8>,
}

/// An owned floating point grayscale image with packed rows. Values range from 0 to 255.
#[derive(Clone, Debug, PartialEq)]
pub struct GrayImageF32 {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// One value per pixel, `width` values per row.
    pub data: Vec<f32>,
}

impl GrayImage {
    /// Returns the value at a pixel, or `None` outside the image.
    pub fn get(&self, x: u32, y: u32) -> Option<u8> {
        (x < self.width && y < self.height)
            .then(|| self.data[y as usize * self.width as usize + x as usize])
    }
}

impl GrayImageF32 {
    /// Returns the value at a pixel, or `None` outside the image.
    pub fn get(&self, x: u32, y: u32) -> Option<f32> {
        (x < self.width && y < self.height)
            .then(|| self.data[y as usize * self.width as usize + x as usize])
    }
}

impl ImageView<'_> {
    /// Converts to 8-bit luma.
    pub fn to_gray(&self) -> GrayImage {
        let mut data = Vec::with_capacity(self.width() as usize * self.height() as usize);
        for row in self.rows() {
            data.extend(row.chunks_exact(BPP).map(|p| luma(p[0], p[1], p[2])));
        }
        GrayImage {
            width: self.width(),
            height: self.height(),
            data,
        }
    }

    /// Converts to floating point luma.
    pub fn to_gray_f32(&self) -> GrayImageF32 {
        let mut data = Vec::new();
        let mut scratch = Vec::new();
        let (width, height) = box_luma_into(self, 1, &mut data, &mut scratch);
        GrayImageF32 {
            width,
            height,
            data,
        }
    }

    /// Converts to floating point luma downscaled by an integer box filter. Partial blocks at the
    /// right and bottom edges are dropped. Output pixel `(x, y)` covers the source block that starts
    /// at `(x * factor, y * factor)`. A factor of zero counts as one.
    pub fn to_gray_f32_downscaled(&self, factor: u32) -> GrayImageF32 {
        let mut data = Vec::new();
        let mut scratch = Vec::new();
        let (width, height) = box_luma_into(self, factor, &mut data, &mut scratch);
        GrayImageF32 {
            width,
            height,
            data,
        }
    }
}

impl super::Image {
    /// Converts to 8-bit luma. See [`ImageView::to_gray`].
    pub fn to_gray(&self) -> GrayImage {
        self.view().to_gray()
    }

    /// Converts to floating point luma. See [`ImageView::to_gray_f32`].
    pub fn to_gray_f32(&self) -> GrayImageF32 {
        self.view().to_gray_f32()
    }
}

/// Writes the box-filtered luma of a view into `out` and returns its width and height. Partial
/// edge blocks are dropped. `scratch` holds the row accumulators between calls.
pub(crate) fn box_luma_into(
    view: &ImageView<'_>,
    factor: u32,
    out: &mut Vec<f32>,
    scratch: &mut Vec<u32>,
) -> (u32, u32) {
    let f = factor.max(1) as usize;
    let ow = view.width() as usize / f;
    let oh = view.height() as usize / f;
    out.clear();
    if ow == 0 || oh == 0 {
        return (0, 0);
    }
    out.reserve(ow * oh);
    let norm = 1.0 / (256.0 * (f * f) as f32);
    scratch.clear();
    scratch.resize(ow, 0);
    for by in 0..oh {
        scratch.fill(0);
        for sy in by * f..(by + 1) * f {
            let row = view.row(sy as u32);
            for (acc, block) in scratch.iter_mut().zip(row.chunks_exact(f * BPP)) {
                *acc += block.chunks_exact(BPP).map(luma_sum).sum::<u32>();
            }
        }
        out.extend(scratch.iter().map(|&s| s as f32 * norm));
    }
    (ow as u32, oh as u32)
}

#[cfg(test)]
mod tests {
    use crate::image::Image;
    use fakeland_types::Rect;

    use super::*;

    #[test]
    fn luma_extremes() {
        assert_eq!(luma(0, 0, 0), 0);
        assert_eq!(luma(255, 255, 255), 255);
        assert_eq!(luma(255, 0, 0), 77);
        assert_eq!(luma(0, 255, 0), 149);
    }

    #[test]
    fn gray_conversions_agree() {
        let mut img = Image::filled(5, 3, [200, 100, 50, 255]);
        img.set_pixel(4, 2, [0, 0, 0, 255]);
        let g = img.view().to_gray();
        let gf = img.view().to_gray_f32();
        assert_eq!(g.get(0, 0), Some(luma(200, 100, 50)));
        assert_eq!(g.get(4, 2), Some(0));
        assert!((gf.get(0, 0).unwrap() - g.get(0, 0).unwrap() as f32).abs() < 0.6);
        assert_eq!(gf.get(5, 0), None);
    }

    #[test]
    fn downscaled_gray_averages_blocks() {
        let mut img = Image::filled(9, 5, [0, 0, 0, 255]);
        img.fill_rect(Rect::new(0, 0, 2, 2), [255, 255, 255, 255]);
        img.fill_rect(Rect::new(3, 0, 1, 2), [255, 255, 255, 255]);
        let g = img.view().to_gray_f32_downscaled(2);
        assert_eq!((g.width, g.height), (4, 2));
        assert!((g.get(0, 0).unwrap() - 255.0).abs() < 1e-3);
        assert!((g.get(1, 0).unwrap() - 127.5).abs() < 1e-3);
        assert!(g.get(0, 1).unwrap().abs() < 1e-3);
    }
}
