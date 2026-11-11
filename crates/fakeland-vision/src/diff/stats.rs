//! Full change statistics of two frames in a region.

use fakeland_types::{Physical, Rect};
use serde::{Deserialize, Serialize};

use super::{DiffError, check_same_size, max_rgb_delta};
use crate::image::{BPP, ImageView};

/// Change statistics of two frames in a region. A pixel changes when its largest red, green or
/// blue difference exceeds the threshold.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DiffStats {
    /// Pixels compared: the area of the region inside the frame.
    pub compared_pixels: u64,
    /// Pixels whose difference exceeds the threshold.
    pub changed_pixels: u64,
    /// Changed pixels divided by compared pixels, zero for an empty region.
    pub changed_fraction: f64,
    /// The smallest rectangle that holds every changed pixel, in frame coordinates.
    pub bbox: Option<Rect<Physical>>,
    /// The largest channel difference in the region.
    pub max_delta: u8,
    /// The mean over compared pixels of the largest channel difference.
    pub mean_delta: f64,
}

/// Compares two frames of the same size in a region. `None` compares the whole frame. The region
/// is clipped to the frame.
pub fn diff<'a, 'b>(
    a: impl Into<ImageView<'a>>,
    b: impl Into<ImageView<'b>>,
    roi: Option<Rect<Physical>>,
    threshold: u8,
) -> Result<DiffStats, DiffError> {
    let (a, b) = (a.into(), b.into());
    check_same_size(&a, &b)?;
    let Some(r) = a.clip(roi) else {
        return Ok(DiffStats::default());
    };
    let (Some(ra), Some(rb)) = (a.region(r), b.region(r)) else {
        return Ok(DiffStats::default());
    };
    let mut changed = 0u64;
    let mut sum = 0u64;
    let mut max_delta = 0u8;
    let (mut x0, mut y0, mut x1, mut y1) = (u32::MAX, u32::MAX, 0u32, 0u32);
    for y in 0..ra.height() {
        let (row_a, row_b) = (ra.row(y), rb.row(y));
        if row_a == row_b {
            continue;
        }
        let mut row_sum = 0u32;
        let (mut first, mut last) = (u32::MAX, 0u32);
        for (x, (pa, pb)) in row_a
            .chunks_exact(BPP)
            .zip(row_b.chunks_exact(BPP))
            .enumerate()
        {
            let d = max_rgb_delta(pa, pb);
            row_sum += d as u32;
            max_delta = max_delta.max(d);
            if d > threshold {
                changed += 1;
                first = first.min(x as u32);
                last = x as u32;
            }
        }
        sum += row_sum as u64;
        if first != u32::MAX {
            x0 = x0.min(first);
            x1 = x1.max(last);
            y0 = y0.min(y);
            y1 = y;
        }
    }
    let compared = r.area() as u64;
    let bbox = (changed > 0).then(|| {
        Rect::new(
            r.x + x0 as i32,
            r.y + y0 as i32,
            (x1 - x0 + 1) as i32,
            (y1 - y0 + 1) as i32,
        )
    });
    Ok(DiffStats {
        compared_pixels: compared,
        changed_pixels: changed,
        changed_fraction: changed as f64 / compared as f64,
        bbox,
        max_delta,
        mean_delta: sum as f64 / compared as f64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;

    #[test]
    fn identical_frames_have_no_change() {
        let a = Image::filled(20, 10, [5, 6, 7, 255]);
        let s = diff(&a, &a, None, 0).unwrap();
        assert_eq!(s.changed_pixels, 0);
        assert_eq!(s.bbox, None);
        assert_eq!(s.compared_pixels, 200);
        assert_eq!(s.mean_delta, 0.0);
    }

    #[test]
    fn reports_bbox_and_deltas() {
        let a = Image::filled(20, 10, [100, 100, 100, 255]);
        let mut b = a.clone();
        b.fill_rect(Rect::new(3, 2, 4, 3), [110, 100, 100, 255]);
        b.set_pixel(15, 8, [100, 100, 160, 255]);
        let s = diff(&a, &b, None, 5).unwrap();
        assert_eq!(s.changed_pixels, 13);
        assert_eq!(s.bbox, Some(Rect::new(3, 2, 13, 7)));
        assert_eq!(s.max_delta, 60);
        assert!((s.mean_delta - (12.0 * 10.0 + 60.0) / 200.0).abs() < 1e-12);
        assert!((s.changed_fraction - 13.0 / 200.0).abs() < 1e-12);
        // The threshold hides small changes.
        let s = diff(&a, &b, None, 10).unwrap();
        assert_eq!(s.changed_pixels, 1);
        assert_eq!(s.bbox, Some(Rect::new(15, 8, 1, 1)));
    }

    #[test]
    fn roi_limits_and_clips() {
        let a = Image::filled(20, 10, [0, 0, 0, 255]);
        let mut b = a.clone();
        b.set_pixel(2, 2, [255, 255, 255, 255]);
        b.set_pixel(18, 9, [255, 255, 255, 255]);
        let s = diff(&a, &b, Some(Rect::new(10, 5, 50, 50)), 0).unwrap();
        assert_eq!(s.compared_pixels, 50);
        assert_eq!(s.changed_pixels, 1);
        assert_eq!(s.bbox, Some(Rect::new(18, 9, 1, 1)));
        let s = diff(&a, &b, Some(Rect::new(30, 30, 5, 5)), 0).unwrap();
        assert_eq!(s, DiffStats::default());
    }

    #[test]
    fn alpha_does_not_count() {
        let a = Image::filled(4, 4, [9, 9, 9, 255]);
        let b = Image::filled(4, 4, [9, 9, 9, 0]);
        assert_eq!(diff(&a, &b, None, 0).unwrap().changed_pixels, 0);
    }

    #[test]
    fn size_mismatch_fails() {
        let a = Image::new(4, 4);
        let b = Image::new(4, 5);
        assert!(matches!(
            diff(&a, &b, None, 0),
            Err(DiffError::SizeMismatch { .. })
        ));
    }
}
