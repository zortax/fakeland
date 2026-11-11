//! Fast change tests that stop at the first decisive pixel.

use fakeland_types::{Physical, Rect};

use super::{DiffError, check_same_size, max_rgb_delta};
use crate::image::{BPP, ImageView};

/// Returns true when any pixel in the region differs by more than the threshold. `None` tests the
/// whole frame. The region is clipped to the frame. Equal rows cost one memory comparison.
pub fn changed<'a, 'b>(
    a: impl Into<ImageView<'a>>,
    b: impl Into<ImageView<'b>>,
    roi: Option<Rect<Physical>>,
    threshold: u8,
) -> Result<bool, DiffError> {
    let (a, b) = (a.into(), b.into());
    check_same_size(&a, &b)?;
    let Some(r) = a.clip(roi) else {
        return Ok(false);
    };
    let (Some(ra), Some(rb)) = (a.region(r), b.region(r)) else {
        return Ok(false);
    };
    Ok(count_changed_up_to(&ra, &rb, threshold, 1) > 0)
}

/// Counts pixels of two equal-size views that differ by more than the threshold. The count stops
/// once it reaches `limit`.
pub(crate) fn count_changed_up_to(
    a: &ImageView<'_>,
    b: &ImageView<'_>,
    threshold: u8,
    limit: u64,
) -> u64 {
    debug_assert!(a.same_size(b));
    let mut count = 0u64;
    for y in 0..a.height() {
        let (row_a, row_b) = (a.row(y), b.row(y));
        if row_a == row_b {
            continue;
        }
        for (pa, pb) in row_a.chunks_exact(BPP).zip(row_b.chunks_exact(BPP)) {
            if max_rgb_delta(pa, pb) > threshold {
                count += 1;
                if count >= limit {
                    return count;
                }
            }
        }
    }
    count
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;

    #[test]
    fn detects_single_pixel() {
        let a = Image::filled(64, 32, [0, 0, 0, 255]);
        let mut b = a.clone();
        assert!(!changed(&a, &b, None, 0).unwrap());
        b.set_pixel(63, 31, [0, 3, 0, 255]);
        assert!(changed(&a, &b, None, 0).unwrap());
        assert!(!changed(&a, &b, None, 3).unwrap());
        assert!(!changed(&a, &b, Some(Rect::new(0, 0, 63, 32)), 0).unwrap());
        assert!(!changed(&a, &b, Some(Rect::new(100, 0, 5, 5)), 0).unwrap());
    }

    #[test]
    fn count_stops_at_limit() {
        let a = Image::filled(10, 10, [0, 0, 0, 255]);
        let b = Image::filled(10, 10, [50, 0, 0, 255]);
        assert_eq!(count_changed_up_to(&a.view(), &b.view(), 0, 7), 7);
        assert_eq!(count_changed_up_to(&a.view(), &b.view(), 0, 1000), 100);
    }
}
