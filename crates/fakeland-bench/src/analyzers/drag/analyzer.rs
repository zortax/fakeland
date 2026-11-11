//! The drag follow analyzer.

use fakeland::{CaptureSpec, CapturedFrame};
use fakeland_trace::Summary;
use fakeland_vision::{MotionConfig, MotionEstimator};

use super::{DragFollowReport, FollowSample, PointerPath};
use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;

/// Measures how dragged content follows the pointer, from a capture of presented frames.
#[derive(Clone, Debug)]
pub struct DragFollow {
    /// The index of the frame capture to read; `None` takes the first capture of presented
    /// frames, else the first capture.
    pub capture: Option<usize>,
    /// The largest displacement per frame in capture pixels.
    pub max_shift: u32,
}

impl Default for DragFollow {
    fn default() -> Self {
        Self {
            capture: None,
            max_shift: 128,
        }
    }
}

impl DragFollow {
    fn frames<'a>(&self, input: &'a AnalysisInput) -> &'a [CapturedFrame] {
        let pick = match self.capture {
            Some(i) => input.frames.get(i),
            None => input
                .frames
                .iter()
                .find(|c| matches!(c.spec, CaptureSpec::EveryPresented { .. }))
                .or_else(|| input.frames.first()),
        };
        pick.map(|c| c.frames.as_slice()).unwrap_or_default()
    }
}

impl Analyzer for DragFollow {
    type Report = DragFollowReport;

    fn name(&self) -> String {
        "drag_follow".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> DragFollowReport {
        let frames = self.frames(input);
        let path = PointerPath::of_drag(&input.timeline);
        let mut report = DragFollowReport {
            frames: frames.len(),
            motions: path.len(),
            distance_px: 0.0,
            rejected_pairs: 0,
            lag_px: None,
            lag_ms: None,
            follow_ratio: None,
            final_error_px: None,
            samples: Vec::new(),
        };
        let (Some((dir, distance)), Some((start, end))) = (path.direction(), path.span()) else {
            return report;
        };
        report.distance_px = distance;
        let cfg = MotionConfig::with_max_shift(self.max_shift);
        let mut est = MotionEstimator::new();
        let mut content = 0.0;
        let mut last_content = None;
        for pair in frames.windows(2) {
            let (a, b) = (&pair[0], &pair[1]);
            let step = if a.image.as_bytes() == b.image.as_bytes() {
                Some((0.0, 0.0))
            } else {
                est.translation(&a.image, &b.image, None, &cfg)
                    .ok()
                    .map(|m| (m.dx * b.scale, m.dy * b.scale))
            };
            let Some((dx, dy)) = step else {
                report.rejected_pairs += 1;
                continue;
            };
            content += dx * dir.0 + dy * dir.1;
            let presented = b.presented.unwrap_or(b.time);
            last_content = Some(content);
            if presented < start {
                content = 0.0;
                continue;
            }
            if presented > end {
                continue;
            }
            let pointer = path.along_at(presented, dir);
            report.samples.push(FollowSample {
                commit: b.commit,
                presented,
                pointer_px: pointer,
                content_px: content,
                lag_px: pointer - content,
                lag_ms: path
                    .time_reaching(content, dir)
                    .map(|t| presented.since(t).as_millis_f64()),
            });
        }
        let lags: Vec<f64> = report.samples.iter().map(|s| s.lag_px).collect();
        let lag_ms: Vec<f64> = report.samples.iter().filter_map(|s| s.lag_ms).collect();
        report.lag_px = Summary::of(&lags);
        report.lag_ms = Summary::of(&lag_ms);
        report.follow_ratio = last_content.map(|c| c / distance);
        report.final_error_px = last_content.map(|c| (distance - c).abs());
        report
    }
}
