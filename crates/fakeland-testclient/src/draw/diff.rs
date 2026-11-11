//! Frame differences: the bounding box of the pixels that differ between two packed frames of
//! the same size, and the band of rows that differ. Damage and buffer uploads use them.

use fakeland_types::{Physical, Rect, Size};

use super::BYTES_PER_PIXEL;

/// Returns the first and one-past-last row that differ, or `None` for equal frames.
///
/// # Panics
/// Panics when a frame is shorter than `size`.
pub fn changed_rows(a: &[u8], b: &[u8], size: Size<Physical>) -> Option<(usize, usize)> {
    let (stride, rows) = dims(size);
    assert!(a.len() >= stride * rows && b.len() >= stride * rows);
    let row = |y: usize| a[y * stride..(y + 1) * stride] != b[y * stride..(y + 1) * stride];
    let first = (0..rows).find(|&y| row(y))?;
    let last = (first..rows).rev().find(|&y| row(y)).unwrap_or(first);
    Some((first, last + 1))
}

/// Returns the bounding box of the pixels that differ, or `None` for equal frames.
pub fn changed_bbox(a: &[u8], b: &[u8], size: Size<Physical>) -> Option<Rect<Physical>> {
    let (y0, y1) = changed_rows(a, b, size)?;
    let (stride, _) = dims(size);
    let mut x0 = usize::MAX;
    let mut x1 = 0;
    for y in y0..y1 {
        let ra = &a[y * stride..(y + 1) * stride];
        let rb = &b[y * stride..(y + 1) * stride];
        if let Some(first) = first_diff(ra, rb) {
            x0 = x0.min(first / BYTES_PER_PIXEL);
            let last = last_diff(ra, rb).unwrap_or(first);
            x1 = x1.max(last / BYTES_PER_PIXEL + 1);
        }
    }
    Some(Rect::new(
        x0 as i32,
        y0 as i32,
        (x1 - x0) as i32,
        (y1 - y0) as i32,
    ))
}

/// Bytes compared at once before the byte-wise search.
const CHUNK: usize = 64;

/// Returns the index of the first byte that differs.
fn first_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    let (ca, cb) = (a.chunks(CHUNK), b.chunks(CHUNK));
    let (i, (xa, xb)) = ca.zip(cb).enumerate().find(|(_, (x, y))| x != y)?;
    let j = xa.iter().zip(xb).position(|(p, q)| p != q)?;
    Some(i * CHUNK + j)
}

/// Returns the index of the last byte that differs.
fn last_diff(a: &[u8], b: &[u8]) -> Option<usize> {
    let n = a.len().min(b.len());
    let mut end = n;
    while end > 0 {
        let start = end.saturating_sub(CHUNK);
        if a[start..end] != b[start..end] {
            let j = (start..end).rev().find(|&k| a[k] != b[k])?;
            return Some(j);
        }
        end = start;
    }
    None
}

fn dims(size: Size<Physical>) -> (usize, usize) {
    (
        size.w.max(0) as usize * BYTES_PER_PIXEL,
        size.h.max(0) as usize,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: i32, h: i32) -> Vec<u8> {
        vec![0; (w * h) as usize * BYTES_PER_PIXEL]
    }

    fn set(f: &mut [u8], w: i32, x: i32, y: i32) {
        let i = ((y * w + x) as usize) * BYTES_PER_PIXEL;
        f[i] = 1;
    }

    #[test]
    fn equal_frames_have_no_change() {
        let a = frame(8, 8);
        assert_eq!(changed_rows(&a, &a, Size::new(8, 8)), None);
        assert_eq!(changed_bbox(&a, &a, Size::new(8, 8)), None);
    }

    #[test]
    fn bbox_covers_every_change() {
        let a = frame(10, 6);
        let mut b = a.clone();
        set(&mut b, 10, 2, 1);
        set(&mut b, 10, 7, 4);
        set(&mut b, 10, 5, 2);
        let size = Size::new(10, 6);
        assert_eq!(changed_rows(&a, &b, size), Some((1, 5)));
        assert_eq!(changed_bbox(&a, &b, size), Some(Rect::new(2, 1, 6, 4)));
    }

    #[test]
    fn wide_rows_find_both_ends() {
        let w = 300;
        let a = frame(w, 2);
        let mut b = a.clone();
        set(&mut b, w, 3, 0);
        set(&mut b, w, 290, 1);
        set(&mut b, w, 150, 1);
        assert_eq!(
            changed_bbox(&a, &b, Size::new(w, 2)),
            Some(Rect::new(3, 0, 288, 2))
        );
        assert_eq!(first_diff(&a, &b), Some(12));
        assert_eq!(last_diff(&a, &b), Some((300 + 290) * 4));
    }

    #[test]
    fn single_pixel() {
        let a = frame(3, 3);
        let mut b = a.clone();
        set(&mut b, 3, 2, 2);
        assert_eq!(
            changed_bbox(&a, &b, Size::new(3, 3)),
            Some(Rect::new(2, 2, 1, 1))
        );
    }
}
