//! The scroll smoothness analyzer.

use std::collections::BTreeMap;

use fakeland::{CaptureSpec, CapturedFrame, FrameCapture};
use fakeland_trace::stats::median;
use fakeland_vision::{Axis, MotionEstimator, MotionRejection};

use super::expected::{consumed_by, scroll_inputs};
use super::gain::{self, Pair};
use super::measure::displacement;
use super::score::smoothness_score;
use super::{FrameStep, ScrollReport};
use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;

/// Measures scroll smoothness from a frame capture of presented frames.
#[derive(Clone, Debug)]
pub struct ScrollSmoothness {
    /// The scroll axis.
    pub axis: Axis,
    /// The index of the frame capture to read; `None` takes the first capture of presented
    /// frames, else the first capture.
    pub capture: Option<usize>,
    /// The largest displacement per step in capture pixels for the wide search, which runs when
    /// the searches near the expected step, no motion and the previous step all fail.
    pub max_shift: u32,
    /// Content pixels per scroll pixel of the app.
    pub content_factor: f64,
    /// A step stalls when the content moves less than this fraction of the expected step.
    pub stall_ratio: f64,
    /// A step is a double step when the content moves farther than this factor times the
    /// median step.
    pub double_step_ratio: f64,
    /// Expected steps below this many logical pixels do not count as motion.
    pub min_step_px: f64,
}

impl Default for ScrollSmoothness {
    fn default() -> Self {
        Self {
            axis: Axis::Vertical,
            capture: None,
            max_shift: 256,
            content_factor: 1.0,
            stall_ratio: 0.25,
            double_step_ratio: 1.75,
            min_step_px: 0.5,
        }
    }
}

impl ScrollSmoothness {
    /// Returns the analyzer for another axis.
    pub fn axis(mut self, axis: Axis) -> Self {
        self.axis = axis;
        self
    }

    /// Selects a frame capture by index.
    pub fn capture(mut self, index: usize) -> Self {
        self.capture = Some(index);
        self
    }

    fn frames<'a>(&self, input: &'a AnalysisInput) -> &'a [CapturedFrame] {
        let pick = match self.capture {
            Some(i) => input.frames.get(i),
            None => input
                .frames
                .iter()
                .find(|c| matches!(c.spec, CaptureSpec::EveryPresented { .. }))
                .or_else(|| input.frames.first()),
        };
        pick.map(|c: &FrameCapture| c.frames.as_slice())
            .unwrap_or_default()
    }
}

impl Analyzer for ScrollSmoothness {
    type Report = ScrollReport;

    fn name(&self) -> String {
        "scroll_smoothness".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> ScrollReport {
        let frames = self.frames(input);
        let surface = frames
            .first()
            .map(|f| f.surface)
            .or_else(|| input.main_surface());
        let tl = &input.timeline;
        let inputs = scroll_inputs(tl, surface, self.axis);
        let committed =
            |f: &CapturedFrame| tl.frame(f.commit).map(|r| r.committed).unwrap_or(f.time);
        let first_sent = inputs.first().map(|i| i.sent);
        let last_consumed = inputs.iter().filter_map(|i| i.committed).max();
        // The displacement the input asks for between each pair of frames, and whether the pair
        // falls inside the active scroll.
        let plans: Vec<(f64, bool)> = frames
            .windows(2)
            .map(|pair| {
                let (a, b) = (&pair[0], &pair[1]);
                let expected = (consumed_by(&inputs, committed(b))
                    - consumed_by(&inputs, committed(a)))
                    * self.content_factor;
                let tb = b.presented.unwrap_or(b.time);
                let active = match (first_sent, last_consumed) {
                    (Some(first), Some(last)) => tb > first && committed(a) < last,
                    _ => false,
                };
                (expected, active)
            })
            .collect();
        let mut est = MotionEstimator::new();
        let active_pairs: Vec<Pair<'_>> = frames
            .windows(2)
            .zip(&plans)
            .filter(|(_, (_, active))| *active)
            .map(|(pair, (expected, _))| Pair {
                prev: &pair[0],
                next: &pair[1],
                expected: *expected,
            })
            .collect();
        let gain = gain::estimate(&mut est, &active_pairs, self.axis);
        let mut steps = Vec::new();
        let mut rejections = BTreeMap::new();
        let mut previous: Option<f64> = None;
        for (pair, &(expected, active)) in frames.windows(2).zip(&plans) {
            let (a, b) = (&pair[0], &pair[1]);
            let target = expected * gain;
            let hints = [target, 0.0, previous.unwrap_or(target)];
            let wide =
                ((3.0 * target.abs() / b.scale.max(1e-9)).ceil() as u32 + 16).min(self.max_shift);
            let measured = match displacement(&mut est, a, b, self.axis, &hints, wide) {
                Ok(m) => {
                    previous = Some(m);
                    Some(m)
                }
                Err(e) => {
                    *rejections.entry(rejection_name(&e)).or_insert(0) += 1;
                    None
                }
            };
            let ta = a.presented.unwrap_or(a.time);
            let tb = b.presented.unwrap_or(b.time);
            steps.push(FrameStep {
                commit: b.commit,
                presented: b.presented,
                interval_ms: tb.since(ta).as_millis_f64(),
                vblanks_missed: match (a.seq, b.seq) {
                    (Some(x), Some(y)) if y > x => y - x - 1,
                    _ => 0,
                },
                expected_px: expected,
                target_px: target,
                measured_px: measured,
                active,
                stalled: false,
                double_step: false,
            });
        }
        self.classify(&mut steps);
        let mut report = self.report(frames, &inputs, steps, gain);
        report.rejections = rejections;
        report
    }
}

impl ScrollSmoothness {
    fn classify(&self, steps: &mut [FrameStep]) {
        let moving: Vec<f64> = steps
            .iter()
            .filter(|s| s.active)
            .filter_map(|s| s.measured_px.map(f64::abs))
            .filter(|m| *m >= self.min_step_px)
            .collect();
        let typical = median(&moving);
        for s in steps.iter_mut().filter(|s| s.active) {
            let Some(m) = s.measured_px else { continue };
            s.stalled = s.target_px.abs() >= self.min_step_px
                && m.abs() < self.stall_ratio * s.target_px.abs();
            s.double_step = typical.is_some_and(|t| m.abs() > self.double_step_ratio * t);
        }
    }

    fn report(
        &self,
        frames: &[CapturedFrame],
        inputs: &[super::ScrollInput],
        steps: Vec<FrameStep>,
        gain: f64,
    ) -> ScrollReport {
        let active: Vec<&FrameStep> = steps.iter().filter(|s| s.active).collect();
        let measured: Vec<(&FrameStep, f64)> = active
            .iter()
            .filter_map(|s| Some((*s, s.measured_px?)))
            .collect();
        let errors: Vec<f64> = measured.iter().map(|(s, m)| m - s.target_px).collect();
        let target_sum: f64 = measured.iter().map(|(s, _)| s.target_px.abs()).sum();
        let measured_total: f64 = measured.iter().map(|(_, m)| m).sum();
        let expected_measured: f64 = measured.iter().map(|(s, _)| s.expected_px).sum();
        let velocity_errors: Vec<f64> = measured
            .iter()
            .filter(|(s, _)| s.interval_ms > 0.0)
            .map(|(s, m)| (m - s.target_px).abs() / (s.interval_ms / 1000.0))
            .collect();
        let velocities: Vec<f64> = measured
            .iter()
            .filter(|(s, _)| s.interval_ms > 0.0)
            .map(|(s, m)| m / (s.interval_ms / 1000.0))
            .collect();
        let mut per_refresh = Vec::new();
        for (s, m) in &measured {
            per_refresh.extend(std::iter::repeat_n(0.0, s.vblanks_missed as usize));
            per_refresh.push(*m);
        }
        let missed: u64 = active.iter().map(|s| s.vblanks_missed).sum();
        let stalled = active.iter().filter(|s| s.stalled).count();
        let direction = inputs.iter().map(|i| i.amount).sum::<f64>().signum();
        let start_latency_ms = inputs.first().and_then(|first| {
            let moved = steps.iter().find(|s| {
                s.presented.is_some_and(|p| p > first.sent)
                    && s.measured_px
                        .is_some_and(|m| m * direction >= self.min_step_px)
            })?;
            Some(moved.presented?.since(first.sent).as_millis_f64())
        });
        ScrollReport {
            frames: frames.len(),
            inputs: inputs.len(),
            input_total_px: inputs.iter().map(|i| i.amount).sum(),
            expected_total_px: inputs
                .iter()
                .filter(|i| i.commit.is_some())
                .map(|i| i.amount)
                .sum::<f64>()
                * self.content_factor,
            measured_total_px: measured_total,
            tracking_ratio: (expected_measured.abs() > f64::EPSILON)
                .then(|| measured_total / expected_measured),
            content_gain: gain,
            active_steps: active.len(),
            rejected_steps: active.iter().filter(|s| s.measured_px.is_none()).count(),
            missed_vblanks: missed,
            stalled_frames: stalled,
            stalls: missed + stalled as u64,
            double_steps: active.iter().filter(|s| s.double_step).count(),
            step_error_px: fakeland_trace::Summary::of(&errors),
            relative_error: (target_sum > f64::EPSILON)
                .then(|| errors.iter().map(|e| e.abs()).sum::<f64>() / target_sum),
            velocity_px_s: fakeland_trace::Summary::of(&velocities),
            velocity_error_px_s: (!velocity_errors.is_empty())
                .then(|| velocity_errors.iter().sum::<f64>() / velocity_errors.len() as f64),
            start_latency_ms,
            smoothness_score: smoothness_score(&per_refresh),
            rejections: BTreeMap::new(),
            steps,
        }
    }
}

/// Returns the snake case name of a rejection, as its JSON form spells it.
fn rejection_name(e: &MotionRejection) -> String {
    serde_json::to_value(e)
        .ok()
        .and_then(|v| match v {
            serde_json::Value::String(s) => Some(s),
            serde_json::Value::Object(m) => m.keys().next().cloned(),
            _ => None,
        })
        .unwrap_or_else(|| e.to_string())
}
