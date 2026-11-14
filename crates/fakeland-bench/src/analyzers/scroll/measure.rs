//! Content displacement between consecutive captured frames. The search tries a small window
//! around each hint (the expected step, no motion, the previous step) on frames cropped by the
//! hint, keeps the most confident answer, and falls back to a wide search. Small windows keep
//! periodic content such as list rows unambiguous. Periodic content also matches one period away
//! from the true step with almost the same confidence, and content that moves in whole rows steps
//! by one period or by two. So the search also looks between no motion and twice the expected
//! step, and the answer whose shifted frame differs least from the next frame wins: only the true
//! step lines up every pixel, text included. Among answers that differ about equally little, the
//! one nearest the expected step wins.

use fakeland::CapturedFrame;
use fakeland_types::Rect;
use fakeland_vision::{
    Axis, Image, ImageView, Motion, MotionConfig, MotionEstimator, MotionRejection,
};

/// The search window around each hint, in capture pixels.
const WINDOW: u32 = 24;

/// Answers whose mismatch is at most this factor times the least mismatch, plus `TIE_FLOOR`,
/// count as equally good.
const TIE: f64 = 1.15;
const TIE_FLOOR: f64 = 0.05;

/// The most windows between no motion and twice the expected step.
const SPAN_WINDOWS: usize = 16;

/// Returns the content displacement from `prev` to `next` in logical pixels, positive when the
/// content moves the way a positive scroll moves it (up or left). `hints` are candidate
/// displacements in logical pixels; the first is the expected step. Returns the reason when every
/// search fails.
pub fn displacement(
    est: &mut MotionEstimator,
    prev: &CapturedFrame,
    next: &CapturedFrame,
    axis: Axis,
    hints: &[f64],
    max_shift: u32,
) -> Result<f64, MotionRejection> {
    if prev.image.width() != next.image.width() || prev.image.height() != next.image.height() {
        return Err(MotionRejection::SizeMismatch);
    }
    if prev.image.as_bytes() == next.image.as_bytes() {
        return Ok(0.0);
    }
    let scale = if next.scale > 0.0 { next.scale } else { 1.0 };
    let expected = hints.first().copied().unwrap_or(0.0);
    let (found, last_err) = near(est, prev, next, axis, &spanned(hints, scale));
    if let Some(px) = choose(&found, expected / scale, |d| {
        mismatch(&prev.image, &next.image, axis, d)
    }) {
        return Ok(px * scale);
    }
    let wide = MotionConfig {
        max_shift: max_shift.max(WINDOW),
        axis: Some(axis),
        ..MotionConfig::default()
    };
    match est.scroll_1d(&prev.image, &next.image, None, axis, &wide) {
        Ok(m) => Ok(-along(m, axis) * scale),
        Err(_) => Err(last_err),
    }
}

/// Searches a small window around each hint, in logical pixels, and returns the answers as
/// `(displacement, confidence)` in capture pixels, with the reason of the last failed search.
/// Frames of another size give no answers.
pub fn near(
    est: &mut MotionEstimator,
    prev: &CapturedFrame,
    next: &CapturedFrame,
    axis: Axis,
    hints: &[f64],
) -> (Vec<(f64, f64)>, MotionRejection) {
    if prev.image.width() != next.image.width() || prev.image.height() != next.image.height() {
        return (Vec::new(), MotionRejection::SizeMismatch);
    }
    let scale = if next.scale > 0.0 { next.scale } else { 1.0 };
    let mut shifts: Vec<i64> = Vec::new();
    for h in hints.iter().filter(|h| h.is_finite()) {
        let s = (h / scale).round() as i64;
        if !shifts.iter().any(|x| (x - s).abs() < i64::from(WINDOW) / 3) {
            shifts.push(s);
        }
    }
    let cfg = MotionConfig {
        max_shift: WINDOW,
        axis: Some(axis),
        ..MotionConfig::default()
    };
    let mut found = Vec::new();
    let mut last_err = MotionRejection::EmptyRegion;
    for s in shifts {
        match shifted(est, prev, next, axis, s, &cfg) {
            Ok(answer) => found.push(answer),
            Err(e) => last_err = e,
        }
    }
    (found, last_err)
}

/// Returns `hints` with more hints between no motion and twice the first hint, one per search
/// window, so the windows cover that span. A span of more than `SPAN_WINDOWS` windows gets none.
fn spanned(hints: &[f64], scale: f64) -> Vec<f64> {
    let mut all = hints.to_vec();
    let Some(first) = hints.first().copied().filter(|h| h.is_finite()) else {
        return all;
    };
    let pitch = f64::from(WINDOW) * 1.5 * scale;
    let count = (2.0 * first.abs() / pitch).ceil() as usize;
    if count <= SPAN_WINDOWS {
        all.extend((1..=count).map(|k| first.signum() * k as f64 * pitch));
    }
    all
}

/// Returns the answer whose shift gives the least `mismatch`; among answers about as good, the
/// one nearest `expected`. Answers are `(displacement, confidence)` in capture pixels, and an
/// answer without a mismatch counts as the worst. A single answer needs no mismatch.
fn choose(
    found: &[(f64, f64)],
    expected: f64,
    mismatch: impl Fn(i64) -> Option<f64>,
) -> Option<f64> {
    if let [only] = found {
        return Some(only.0);
    }
    let scored: Vec<(f64, f64)> = found
        .iter()
        .map(|a| (a.0, mismatch(a.0.round() as i64).unwrap_or(f64::INFINITY)))
        .collect();
    let best = scored.iter().map(|a| a.1).fold(f64::INFINITY, f64::min);
    scored
        .iter()
        .filter(|a| a.1 <= best * TIE + TIE_FLOOR)
        .min_by(|a, b| (a.0 - expected).abs().total_cmp(&(b.0 - expected).abs()))
        .map(|a| a.0)
}

fn along(m: Motion, axis: Axis) -> f64 {
    match axis {
        Axis::Vertical => m.dy,
        Axis::Horizontal => m.dx,
    }
}

/// Estimates the displacement in capture pixels near `shift` on frames cropped by `shift`, with
/// its confidence.
fn shifted(
    est: &mut MotionEstimator,
    prev: &CapturedFrame,
    next: &CapturedFrame,
    axis: Axis,
    shift: i64,
    cfg: &MotionConfig,
) -> Result<(f64, f64), MotionRejection> {
    let (w, h) = (
        i64::from(prev.image.width()),
        i64::from(prev.image.height()),
    );
    let len = match axis {
        Axis::Vertical => h,
        Axis::Horizontal => w,
    };
    let s = shift.clamp(-(len - 1), len - 1);
    let keep = (len - s.abs()) as i32;
    // Content moves by `-s` along the axis: prev at `s` lines up with next at 0.
    let (a0, b0) = if s >= 0 {
        (s as i32, 0)
    } else {
        (0, (-s) as i32)
    };
    let rect = |start: i32| match axis {
        Axis::Vertical => Rect::new(0, start, w as i32, keep),
        Axis::Horizontal => Rect::new(start, 0, keep, h as i32),
    };
    let pa: ImageView<'_> = prev
        .image
        .view()
        .region(rect(a0))
        .ok_or(MotionRejection::EmptyRegion)?;
    let pb: ImageView<'_> = next
        .image
        .view()
        .region(rect(b0))
        .ok_or(MotionRejection::EmptyRegion)?;
    let m = est.scroll_1d(pa, pb, None, axis, cfg)?;
    Ok((s as f64 - along(m, axis), m.confidence))
}

/// Returns the mean absolute color difference between `next` and `prev` moved by `shift` capture
/// pixels along the axis (content moves by `-shift`, so `prev` at `shift` lines up with `next` at
/// 0), over their overlap. Every second line and pixel counts. Returns `None` when the overlap is
/// less than a third of the frame or the frames differ in size.
pub fn mismatch(prev: &Image, next: &Image, axis: Axis, shift: i64) -> Option<f64> {
    if prev.width() != next.width() || prev.height() != next.height() {
        return None;
    }
    let (w, h) = (i64::from(prev.width()), i64::from(prev.height()));
    let len = match axis {
        Axis::Vertical => h,
        Axis::Horizontal => w,
    };
    if 3 * (len - shift.abs()) < len {
        return None;
    }
    let (a, b) = (prev.as_bytes(), next.as_bytes());
    let (sa, sb) = (prev.stride(), next.stride());
    let (dx, dy) = match axis {
        Axis::Vertical => (0, shift),
        Axis::Horizontal => (shift, 0),
    };
    let mut sum = 0u64;
    let mut count = 0u64;
    for y in (0..h).step_by(2) {
        let ya = y + dy;
        if !(0..h).contains(&ya) {
            continue;
        }
        for x in (0..w).step_by(2) {
            let xa = x + dx;
            if !(0..w).contains(&xa) {
                continue;
            }
            let pa = ya as usize * sa + xa as usize * 4;
            let pb = y as usize * sb + x as usize * 4;
            for c in 0..3 {
                sum += u64::from(a[pa + c].abs_diff(b[pb + c]));
            }
            count += 3;
        }
    }
    (count > 0).then(|| sum as f64 / count as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_least_mismatch_wins_and_ties_go_to_the_expected_step() {
        let by = |table: &'static [(i64, f64)]| {
            move |d: i64| table.iter().find(|t| t.0 == d).map(|t| t.1)
        };
        // The true step and its alias one row period away differ about equally little.
        let tie = by(&[(5, 1.0), (-27, 1.1)]);
        assert_eq!(
            choose(&[(5.0, 0.95), (-27.0, 0.9)], -26.7, tie),
            Some(-27.0)
        );
        // A clearly better answer wins over a closer one.
        let clear = by(&[(5, 1.0), (-27, 9.0)]);
        assert_eq!(
            choose(&[(5.0, 0.95), (-27.0, 0.9)], -26.7, clear),
            Some(5.0)
        );
        assert_eq!(choose(&[(4.0, 0.5)], 9.0, |_| None), Some(4.0));
        assert_eq!(choose(&[], 3.0, |_| None), None);
    }
}
