//! Comparison of reported damage with the real pixel change between two frames.

use fakeland_types::{Physical, Rect};
use serde::{Deserialize, Serialize};

use super::coverage::bands;
use crate::diff::{DiffError, check_same_size, max_rgb_delta};
use crate::image::{BPP, ImageView};

/// Accuracy of reported damage against the pixels that changed. Areas are in pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DamageAccuracy {
    /// Pixels inside the union of the damage rectangles, clipped to the frame.
    pub reported_area: u64,
    /// Pixels that changed.
    pub changed_area: u64,
    /// Changed pixels inside the reported damage.
    pub changed_inside: u64,
    /// Reported pixels per changed pixel. With no changed pixel it equals the reported area. One
    /// means exact damage; a large value means wasted repaint.
    pub over_damage_ratio: f64,
    /// Reported pixels that did not change, as a fraction of the reported area. Zero without
    /// reported damage.
    pub unchanged_damage_fraction: f64,
    /// Changed pixels outside the reported damage. A value above zero is a client bug: the
    /// compositor can show stale content there.
    pub under_damage_pixels: u64,
    /// The smallest rectangle that holds every changed pixel.
    pub changed_bbox: Option<Rect<Physical>>,
    /// The smallest rectangle that holds every changed pixel outside the reported damage.
    pub under_damage_bbox: Option<Rect<Physical>>,
}

/// Bounds of a set of pixels, grown one pixel at a time.
#[derive(Clone, Copy)]
struct Bounds {
    x0: u32,
    y0: u32,
    x1: u32,
    y1: u32,
}

impl Bounds {
    const EMPTY: Bounds = Bounds {
        x0: u32::MAX,
        y0: u32::MAX,
        x1: 0,
        y1: 0,
    };

    fn add_span(&mut self, y: u32, first: u32, last: u32) {
        self.x0 = self.x0.min(first);
        self.x1 = self.x1.max(last);
        self.y0 = self.y0.min(y);
        self.y1 = self.y1.max(y);
    }

    fn rect(self) -> Option<Rect<Physical>> {
        (self.x0 != u32::MAX).then(|| {
            Rect::new(
                self.x0 as i32,
                self.y0 as i32,
                (self.x1 - self.x0 + 1) as i32,
                (self.y1 - self.y0 + 1) as i32,
            )
        })
    }
}

/// Compares the damage reported for the change from `prev` to `next` with the pixels that changed
/// by more than `threshold`. Damage rectangles are in frame pixels and may overlap.
pub fn damage_accuracy<'a, 'b>(
    prev: impl Into<ImageView<'a>>,
    next: impl Into<ImageView<'b>>,
    damage: &[Rect<Physical>],
    threshold: u8,
) -> Result<DamageAccuracy, DiffError> {
    let (prev, next) = (prev.into(), next.into());
    check_same_size(&prev, &next)?;
    let frame = prev.bounds();
    let mut reported = 0u64;
    let mut inside = 0u64;
    let mut outside = 0u64;
    let mut all = Bounds::EMPTY;
    let mut under = Bounds::EMPTY;
    for band in bands(frame, damage) {
        reported += (band.y1 - band.y0) as u64 * band.covered_width() as u64;
        for y in band.y0..band.y1 {
            let (row_a, row_b) = (prev.row(y), next.row(y));
            if row_a == row_b {
                continue;
            }
            for seg in &band.segments {
                let range = seg.start as usize * BPP..seg.end as usize * BPP;
                let pixels = row_a[range.clone()]
                    .chunks_exact(BPP)
                    .zip(row_b[range].chunks_exact(BPP));
                let (mut count, mut first, mut last) = (0u64, u32::MAX, 0u32);
                for (i, (pa, pb)) in pixels.enumerate() {
                    if max_rgb_delta(pa, pb) > threshold {
                        count += 1;
                        first = first.min(seg.start + i as u32);
                        last = seg.start + i as u32;
                    }
                }
                if count == 0 {
                    continue;
                }
                all.add_span(y, first, last);
                if seg.covered {
                    inside += count;
                } else {
                    outside += count;
                    under.add_span(y, first, last);
                }
            }
        }
    }
    let changed = inside + outside;
    Ok(DamageAccuracy {
        reported_area: reported,
        changed_area: changed,
        changed_inside: inside,
        over_damage_ratio: reported as f64 / changed.max(1) as f64,
        unchanged_damage_fraction: if reported == 0 {
            0.0
        } else {
            (reported - inside) as f64 / reported as f64
        },
        under_damage_pixels: outside,
        changed_bbox: all.rect(),
        under_damage_bbox: under.rect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::Image;

    fn frames() -> (Image, Image) {
        let a = Image::filled(100, 80, [20, 20, 20, 255]);
        let mut b = a.clone();
        b.fill_rect(Rect::new(10, 10, 10, 10), [200, 20, 20, 255]);
        (a, b)
    }

    #[test]
    fn exact_damage() {
        let (a, b) = frames();
        let r = damage_accuracy(&a, &b, &[Rect::new(10, 10, 10, 10)], 0).unwrap();
        assert_eq!(r.reported_area, 100);
        assert_eq!(r.changed_area, 100);
        assert_eq!(r.changed_inside, 100);
        assert_eq!(r.under_damage_pixels, 0);
        assert_eq!(r.over_damage_ratio, 1.0);
        assert_eq!(r.unchanged_damage_fraction, 0.0);
        assert_eq!(r.changed_bbox, Some(Rect::new(10, 10, 10, 10)));
        assert_eq!(r.under_damage_bbox, None);
    }

    #[test]
    fn full_frame_damage_over_damages() {
        let (a, b) = frames();
        let r = damage_accuracy(&a, &b, &[Rect::new(0, 0, 100, 80)], 0).unwrap();
        assert_eq!(r.reported_area, 8000);
        assert_eq!(r.over_damage_ratio, 80.0);
        assert!((r.unchanged_damage_fraction - 7900.0 / 8000.0).abs() < 1e-12);
        assert_eq!(r.under_damage_pixels, 0);
    }

    #[test]
    fn partial_damage_under_damages() {
        let (a, b) = frames();
        // The damage covers the left half of the change and an unrelated area.
        let r = damage_accuracy(
            &a,
            &b,
            &[Rect::new(10, 10, 5, 10), Rect::new(50, 50, 10, 10)],
            0,
        )
        .unwrap();
        assert_eq!(r.reported_area, 150);
        assert_eq!(r.changed_inside, 50);
        assert_eq!(r.under_damage_pixels, 50);
        assert_eq!(r.under_damage_bbox, Some(Rect::new(15, 10, 5, 10)));
        assert_eq!(r.changed_bbox, Some(Rect::new(10, 10, 10, 10)));
    }

    #[test]
    fn overlapping_and_out_of_frame_damage() {
        let (a, b) = frames();
        let r = damage_accuracy(
            &a,
            &b,
            &[
                Rect::new(5, 5, 20, 20),
                Rect::new(10, 10, 20, 20),
                Rect::new(90, 70, 50, 50),
            ],
            0,
        )
        .unwrap();
        assert_eq!(r.reported_area, 400 + 400 - 15 * 15 + 100);
        assert_eq!(r.under_damage_pixels, 0);
    }

    #[test]
    fn no_change_no_damage() {
        let a = Image::filled(10, 10, [1, 2, 3, 255]);
        let r = damage_accuracy(&a, &a, &[], 0).unwrap();
        assert_eq!(r, DamageAccuracy::default());
        let r = damage_accuracy(&a, &a, &[Rect::new(0, 0, 4, 4)], 0).unwrap();
        assert_eq!(r.over_damage_ratio, 16.0);
        assert_eq!(r.unchanged_damage_fraction, 1.0);
    }

    #[test]
    fn missing_damage_is_all_under() {
        let (a, b) = frames();
        let r = damage_accuracy(&a, &b, &[], 0).unwrap();
        assert_eq!(r.under_damage_pixels, 100);
        assert_eq!(r.reported_area, 0);
        assert_eq!(r.under_damage_bbox, Some(Rect::new(10, 10, 10, 10)));
    }
}
