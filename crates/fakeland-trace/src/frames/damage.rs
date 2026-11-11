//! The covered area of a set of damage rectangles.

use fakeland_types::{Physical, Rect};

/// The rectangle count above which the area is the clipped sum of the areas.
const EXACT_LIMIT: usize = 256;

/// Returns the area that the union of `rects` covers, clipped to `bounds` when given.
///
/// Overlaps count once. Above 256 rectangles the result is the sum of the clipped areas, capped
/// at the bounds area.
pub fn union_area(rects: &[Rect<Physical>], bounds: Option<Rect<Physical>>) -> i64 {
    let clipped: Vec<Span> = rects
        .iter()
        .filter_map(|r| clip(Span::of(*r), bounds.map(Span::of)))
        .collect();
    if clipped.len() > EXACT_LIMIT {
        let sum: i64 = clipped.iter().map(Span::area).sum();
        return bounds.map_or(sum, |b| sum.min(Span::of(b).area()));
    }
    let mut xs: Vec<i64> = clipped.iter().flat_map(|r| [r.x0, r.x1]).collect();
    xs.sort_unstable();
    xs.dedup();
    let mut area = 0;
    for pair in xs.windows(2) {
        let (x0, x1) = (pair[0], pair[1]);
        let mut spans: Vec<(i64, i64)> = clipped
            .iter()
            .filter(|r| r.x0 <= x0 && r.x1 >= x1)
            .map(|r| (r.y0, r.y1))
            .collect();
        spans.sort_unstable();
        let mut covered = 0;
        let mut end = i64::MIN;
        for (y0, y1) in spans {
            let start = y0.max(end);
            if y1 > start {
                covered += y1 - start;
            }
            end = end.max(y1);
        }
        area += covered * (x1 - x0);
    }
    area
}

/// A rectangle by its edges in 64-bit units, so `x + w` cannot overflow.
#[derive(Clone, Copy)]
struct Span {
    x0: i64,
    y0: i64,
    x1: i64,
    y1: i64,
}

impl Span {
    fn of(r: Rect<Physical>) -> Self {
        let (x, y) = (i64::from(r.x), i64::from(r.y));
        Self {
            x0: x,
            y0: y,
            x1: x + i64::from(r.w),
            y1: y + i64::from(r.h),
        }
    }

    fn area(&self) -> i64 {
        (self.x1 - self.x0) * (self.y1 - self.y0)
    }
}

fn clip(r: Span, bounds: Option<Span>) -> Option<Span> {
    let r = match bounds {
        Some(b) => Span {
            x0: r.x0.max(b.x0),
            y0: r.y0.max(b.y0),
            x1: r.x1.min(b.x1),
            y1: r.y1.min(b.y1),
        },
        None => r,
    };
    (r.x1 > r.x0 && r.y1 > r.y0).then_some(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlaps_count_once() {
        let rects = [Rect::new(0, 0, 10, 10), Rect::new(5, 5, 10, 10)];
        assert_eq!(union_area(&rects, None), 175);
    }

    #[test]
    fn full_damage_clips_to_the_buffer() {
        let rects = [Rect::new(0, 0, i32::MAX, i32::MAX)];
        assert_eq!(union_area(&rects, Some(Rect::new(0, 0, 100, 50))), 5000);
        let offset = [Rect::new(7, 3, i32::MAX, i32::MAX)];
        assert_eq!(union_area(&offset, Some(Rect::new(0, 0, 10, 10))), 21);
    }

    #[test]
    fn disjoint_and_empty() {
        let rects = [
            Rect::new(0, 0, 2, 2),
            Rect::new(10, 10, 3, 3),
            Rect::new(4, 4, 0, 9),
        ];
        assert_eq!(union_area(&rects, None), 13);
        assert_eq!(union_area(&[], None), 0);
    }

    #[test]
    fn many_rects_use_the_capped_sum() {
        let rects: Vec<_> = (0..300).map(|i| Rect::new(i, 0, 1, 1)).collect();
        assert_eq!(union_area(&rects, None), 300);
        assert_eq!(union_area(&rects, Some(Rect::new(0, 0, 10, 10))), 10);
    }
}
