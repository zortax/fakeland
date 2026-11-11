//! Row coverage of a set of rectangles. Rows split into bands where the covered spans stay
//! constant; each band lists the covered and uncovered segments of a row in order.

use fakeland_types::{Physical, Rect};

/// A horizontal segment of a row, half open, in region coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Segment {
    /// First column.
    pub start: u32,
    /// Column after the last.
    pub end: u32,
    /// True when a rectangle covers the segment.
    pub covered: bool,
}

/// A run of rows that share one segment list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Band {
    /// First row, in region coordinates.
    pub y0: u32,
    /// Row after the last.
    pub y1: u32,
    /// Segments that tile the row from column zero to the region width.
    pub segments: Vec<Segment>,
}

impl Band {
    /// Returns the covered pixels of one row.
    pub(crate) fn covered_width(&self) -> u32 {
        self.segments
            .iter()
            .filter(|s| s.covered)
            .map(|s| s.end - s.start)
            .sum()
    }
}

/// Splits a region into bands of constant coverage by the rectangles. Rectangles are in the
/// coordinates of the frame that holds `region`.
pub(crate) fn bands(region: Rect<Physical>, rects: &[Rect<Physical>]) -> Vec<Band> {
    let local: Vec<Rect<Physical>> = rects
        .iter()
        .filter_map(|r| r.intersection(region))
        .map(|r| r.translate(-region.x, -region.y))
        .collect();
    let mut cuts: Vec<i32> = vec![0, region.h];
    for r in &local {
        cuts.push(r.y);
        cuts.push(r.y + r.h);
    }
    cuts.sort_unstable();
    cuts.dedup();
    let mut spans: Vec<(i32, i32)> = Vec::new();
    let mut out: Vec<Band> = Vec::with_capacity(cuts.len());
    for pair in cuts.windows(2) {
        let (y0, y1) = (pair[0], pair[1]);
        spans.clear();
        spans.extend(
            local
                .iter()
                .filter(|r| r.y <= y0 && r.y + r.h >= y1)
                .map(|r| (r.x, r.x + r.w)),
        );
        let segments = segments(&mut spans, region.w);
        match out.last_mut() {
            Some(last) if last.segments == segments => last.y1 = y1 as u32,
            _ => out.push(Band {
                y0: y0 as u32,
                y1: y1 as u32,
                segments,
            }),
        }
    }
    out
}

/// Merges spans and returns the segments that tile `0..width`.
fn segments(spans: &mut [(i32, i32)], width: i32) -> Vec<Segment> {
    spans.sort_unstable();
    let mut out = Vec::new();
    let mut x = 0;
    let mut i = 0;
    while i < spans.len() {
        let (start, mut end) = spans[i];
        i += 1;
        while i < spans.len() && spans[i].0 <= end {
            end = end.max(spans[i].1);
            i += 1;
        }
        if start > x {
            out.push(seg(x, start, false));
        }
        out.push(seg(start, end, true));
        x = end;
    }
    if x < width {
        out.push(seg(x, width, false));
    }
    out
}

fn seg(start: i32, end: i32, covered: bool) -> Segment {
    Segment {
        start: start as u32,
        end: end as u32,
        covered,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(bands: &[Band]) -> u32 {
        bands
            .iter()
            .map(|b| (b.y1 - b.y0) * b.covered_width())
            .sum()
    }

    #[test]
    fn overlapping_rects_count_once() {
        let region = Rect::new(0, 0, 100, 50);
        let rects = [
            Rect::new(10, 10, 20, 20),
            Rect::new(20, 20, 20, 20),
            Rect::new(10, 10, 5, 5),
        ];
        let b = bands(region, &rects);
        assert_eq!(area(&b), 400 + 400 - 100);
        assert_eq!(b.first().map(|b| b.y0), Some(0));
        assert_eq!(b.last().map(|b| b.y1), Some(50));
        for band in &b {
            assert_eq!(band.segments.first().map(|s| s.start), Some(0));
            assert_eq!(band.segments.last().map(|s| s.end), Some(100));
        }
    }

    #[test]
    fn rects_clip_to_region() {
        let region = Rect::new(50, 50, 10, 10);
        let rects = [Rect::new(0, 0, 55, 55), Rect::new(58, 0, 100, 100)];
        let b = bands(region, &rects);
        assert_eq!(area(&b), 25 + 20);
    }

    #[test]
    fn no_rects_one_uncovered_band() {
        let b = bands(Rect::new(0, 0, 8, 4), &[]);
        assert_eq!(b.len(), 1);
        assert_eq!(
            b[0].segments,
            vec![Segment {
                start: 0,
                end: 8,
                covered: false
            }]
        );
    }

    #[test]
    fn touching_spans_merge() {
        let b = bands(
            Rect::new(0, 0, 30, 1),
            &[Rect::new(0, 0, 10, 1), Rect::new(10, 0, 10, 1)],
        );
        assert_eq!(b[0].segments.len(), 2);
        assert_eq!(b[0].covered_width(), 20);
    }
}
