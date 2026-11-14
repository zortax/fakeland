//! The content gain of a scroll: content pixels per scroll pixel of the input. Apps scale scroll
//! input by fixed factors, and the step search needs the scaled step as its hint. On active steps
//! from the middle of the scroll, windowed searches near the expected step times each candidate
//! gain give answers. The answer whose shifted frame differs least from the next frame is the
//! step of a sample. The gain is the summed steps over the summed expected steps of the samples
//! near the median sample ratio, so input that lands one event early or late in a frame evens
//! out. Content that repeats, such as list rows, also matches one period away, but only the true
//! step lines up every pixel, text included.

use fakeland::CapturedFrame;
use fakeland_trace::stats::median;
use fakeland_vision::{Axis, MotionEstimator};

use super::measure::{mismatch, near};

/// The most steps that the estimate reads.
const SAMPLES: usize = 16;
/// The candidate gains, in steps of `GAIN_STEP` up to `MAX_GAIN`.
const GAIN_STEP: f64 = 0.25;
const MAX_GAIN: f64 = 16.0;
/// Expected steps below this many logical pixels give no sample.
const MIN_STEP_PX: f64 = 2.0;
/// A gain this close to 1 counts as 1.
const UNITY: f64 = 0.1;
/// Samples whose ratio is farther than this factor from the median ratio stay out of the sums.
const OUTLIER: f64 = 2.0;

/// Two consecutive frames of the active scroll and the displacement the input asks for between
/// them, in logical pixels.
pub struct Pair<'a> {
    pub prev: &'a CapturedFrame,
    pub next: &'a CapturedFrame,
    pub expected: f64,
}

/// Returns the content gain of the active pairs of a scroll, or 1 when no pair gives a sample.
/// The estimate skips the first quarter of the pairs, where an app that animates its scroll
/// still catches up with the input, and spreads its samples over the rest.
pub fn estimate(est: &mut MotionEstimator, pairs: &[Pair<'_>], axis: Axis) -> f64 {
    let usable: Vec<&Pair<'_>> = pairs
        .iter()
        .skip(pairs.len() / 4)
        .filter(|p| p.expected.abs() >= MIN_STEP_PX)
        .filter(|p| p.prev.image.as_bytes() != p.next.image.as_bytes())
        .collect();
    let stride = usable.len().div_ceil(SAMPLES).max(1);
    let candidates = (1..=(MAX_GAIN / GAIN_STEP) as usize).map(|k| k as f64 * GAIN_STEP);
    let mut samples: Vec<(f64, f64)> = Vec::new();
    for p in usable.into_iter().step_by(stride) {
        let hints: Vec<f64> = candidates.clone().map(|g| g * p.expected).collect();
        let scale = if p.next.scale > 0.0 {
            p.next.scale
        } else {
            1.0
        };
        let (found, _) = near(est, p.prev, p.next, axis, &hints);
        let mut shifts: Vec<i64> = found.iter().map(|a| a.0.round() as i64).collect();
        shifts.sort_unstable();
        shifts.dedup();
        let best = shifts
            .into_iter()
            .filter_map(|d| Some((d, mismatch(&p.prev.image, &p.next.image, axis, d)?)))
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((d, _)) = best {
            let step = d as f64 * scale;
            if step / p.expected > 0.0 {
                samples.push((step, p.expected));
            }
        }
    }
    let ratios: Vec<f64> = samples.iter().map(|(m, e)| m / e).collect();
    let Some(typical) = median(&ratios) else {
        return 1.0;
    };
    let near: Vec<&(f64, f64)> = samples
        .iter()
        .filter(|(m, e)| {
            let r = m / e;
            r <= typical * OUTLIER && r >= typical / OUTLIER
        })
        .collect();
    let expected: f64 = near.iter().map(|(_, e)| e).sum();
    let gain = near.iter().map(|(m, _)| m).sum::<f64>() / expected;
    if !gain.is_finite() || (gain - 1.0).abs() <= UNITY {
        1.0
    } else {
        gain.min(MAX_GAIN)
    }
}
