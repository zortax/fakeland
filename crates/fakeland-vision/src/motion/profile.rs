//! Band projection profiles: the mean luma of each line along a motion axis, computed separately
//! in bands across the axis. Bands keep the layout of text lines apart, so lines of equal ink
//! still differ.

use super::Axis;
use crate::image::{BPP, ImageView, luma_sum};

/// Writes `bands` profiles of a view along an axis into `out`, band after band, each centered on
/// its mean. Returns the profile length. A vertical axis gives one value per row; a horizontal
/// axis gives one value per column.
pub(crate) fn band_profiles(
    view: &ImageView<'_>,
    axis: Axis,
    bands: usize,
    out: &mut Vec<f32>,
) -> usize {
    let (w, h) = (view.width() as usize, view.height() as usize);
    let bands = bands.max(1);
    out.clear();
    let len = match axis {
        Axis::Vertical => {
            out.resize(bands * h, 0.0);
            for y in 0..h {
                let row = view.row(y as u32);
                for b in 0..bands {
                    let (x0, x1) = (b * w / bands, (b + 1) * w / bands);
                    let sum: u32 = row[x0 * BPP..x1 * BPP]
                        .chunks_exact(BPP)
                        .map(luma_sum)
                        .sum();
                    out[b * h + y] = sum as f32 / (256.0 * (x1 - x0) as f32);
                }
            }
            h
        }
        Axis::Horizontal => {
            out.resize(bands * w, 0.0);
            for b in 0..bands {
                let (y0, y1) = (b * h / bands, (b + 1) * h / bands);
                let profile = &mut out[b * w..(b + 1) * w];
                for y in y0..y1 {
                    let row = view.row(y as u32);
                    for (v, px) in profile.iter_mut().zip(row.chunks_exact(BPP)) {
                        *v += luma_sum(px) as f32;
                    }
                }
                let norm = 1.0 / (256.0 * (y1 - y0) as f32);
                profile.iter_mut().for_each(|v| *v *= norm);
            }
            w
        }
    };
    for profile in out.chunks_exact_mut(len) {
        let mean = profile.iter().map(|&v| v as f64).sum::<f64>() / len as f64;
        profile.iter_mut().for_each(|v| *v -= mean as f32);
    }
    len
}

/// Returns the largest standard deviation among the band profiles.
pub(crate) fn max_band_std(profiles: &[f32], len: usize) -> f64 {
    profiles
        .chunks_exact(len)
        .map(|p| (p.iter().map(|&v| v as f64 * v as f64).sum::<f64>() / len as f64).sqrt())
        .fold(0.0, f64::max)
}

#[cfg(test)]
mod tests {
    use fakeland_types::Rect;

    use super::*;
    use crate::image::Image;

    #[test]
    fn vertical_bands_see_rows() {
        let mut img = Image::filled(8, 4, [0, 0, 0, 255]);
        img.fill_rect(Rect::new(0, 1, 4, 1), [255, 255, 255, 255]);
        let mut out = Vec::new();
        let len = band_profiles(&img.view(), Axis::Vertical, 2, &mut out);
        assert_eq!(len, 4);
        // Band 0 has a bright row 1; band 1 is flat.
        assert!((out[1] - (255.0 - 63.75)).abs() < 1e-3);
        assert!((out[0] + 63.75).abs() < 1e-3);
        assert!(out[4..].iter().all(|v| v.abs() < 1e-6));
        assert!(max_band_std(&out, len) > 100.0);
    }

    #[test]
    fn horizontal_bands_see_columns() {
        let mut img = Image::filled(6, 4, [0, 0, 0, 255]);
        img.fill_rect(Rect::new(2, 0, 1, 2), [255, 255, 255, 255]);
        let mut out = Vec::new();
        let len = band_profiles(&img.view(), Axis::Horizontal, 2, &mut out);
        assert_eq!(len, 6);
        assert!((out[2] - (255.0 - 42.5)).abs() < 1e-3);
        assert!(out[6..].iter().all(|v| v.abs() < 1e-6));
    }
}
