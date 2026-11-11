//! Content displacement between consecutive captured frames. The search tries a small window
//! around each hint (the expected step, no motion, the previous step) on frames cropped by the
//! hint, keeps the most confident answer, and falls back to a wide search. Small windows keep
//! periodic content such as list rows unambiguous. Periodic content also matches one period away
//! from the true step with almost the same confidence, so among answers close to the best
//! confidence the one nearest the expected step wins.

use fakeland::CapturedFrame;
use fakeland_types::Rect;
use fakeland_vision::{Axis, ImageView, Motion, MotionConfig, MotionEstimator, MotionRejection};

/// The search window around each hint, in capture pixels.
const WINDOW: u32 = 24;

/// Answers with at least this share of the best confidence count as equally confident.
const TIE: f64 = 0.85;

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
    let mut found: Vec<(f64, f64)> = Vec::new();
    let mut last_err = MotionRejection::EmptyRegion;
    for s in shifts {
        match shifted(est, prev, next, axis, s, &cfg) {
            Ok(answer) => found.push(answer),
            Err(e) => last_err = e,
        }
    }
    if let Some(px) = choose(&found, hints.first().copied().unwrap_or(0.0) / scale) {
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

/// Returns the answer nearest `expected` among the answers close to the best confidence.
/// Answers are `(displacement, confidence)` in capture pixels.
fn choose(found: &[(f64, f64)], expected: f64) -> Option<f64> {
    let best = found.iter().map(|a| a.1).fold(f64::NEG_INFINITY, f64::max);
    found
        .iter()
        .filter(|a| a.1 >= best * TIE)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn near_ties_go_to_the_expected_step() {
        // The true step and its alias one row period away.
        assert_eq!(choose(&[(5.0, 0.95), (-27.0, 0.9)], -26.7), Some(-27.0));
        // A clearly better answer wins over a closer one.
        assert_eq!(choose(&[(5.0, 0.95), (-27.0, 0.3)], -26.7), Some(5.0));
        assert_eq!(choose(&[], 3.0), None);
    }
}
