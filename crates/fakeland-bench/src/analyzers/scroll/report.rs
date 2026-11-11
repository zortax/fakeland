//! The scroll smoothness report.

use std::collections::BTreeMap;

use fakeland_trace::Summary;
use fakeland_types::{CommitId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// The step from one captured frame to the next.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameStep {
    /// The commit of the later frame.
    pub commit: CommitId,
    /// The presentation time of the later frame.
    pub presented: Option<Timestamp>,
    /// The time since the earlier frame.
    pub interval_ms: f64,
    /// Refreshes between the two frames that showed no new frame.
    pub vblanks_missed: u64,
    /// The displacement the consumed scroll events ask for, in logical pixels.
    pub expected_px: f64,
    /// The content displacement in logical pixels; `None` when motion estimation rejected the
    /// pair.
    pub measured_px: Option<f64>,
    /// The step falls inside the active scroll.
    pub active: bool,
    /// The content stood still while the input moved.
    pub stalled: bool,
    /// The content moved much farther than a median step.
    pub double_step: bool,
}

/// The smoothness of one scroll.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScrollReport {
    /// The captured frames.
    pub frames: usize,
    /// The scroll events along the axis.
    pub inputs: usize,
    /// The scroll the events ask for, in logical pixels.
    pub input_total_px: f64,
    /// The scroll the consumed events ask for, times the content factor.
    pub expected_total_px: f64,
    /// The content displacement over the measured steps.
    pub measured_total_px: f64,
    /// Measured over expected displacement on measured steps.
    pub tracking_ratio: Option<f64>,
    /// Steps inside the active scroll.
    pub active_steps: usize,
    /// Steps whose pair motion estimation rejected.
    pub rejected_steps: usize,
    /// Refreshes inside the active scroll that showed an old frame.
    pub missed_vblanks: u64,
    /// Presented frames whose content stood still while the input moved.
    pub stalled_frames: usize,
    /// Missed vblanks plus stalled frames.
    pub stalls: u64,
    /// Steps much farther than the median step.
    pub double_steps: usize,
    /// Measured minus expected displacement per step.
    pub step_error_px: Option<Summary>,
    /// The sum of absolute step errors over the sum of expected steps.
    pub relative_error: Option<f64>,
    /// The content velocity per step.
    pub velocity_px_s: Option<Summary>,
    /// The mean of `|measured - expected| / interval` over measured steps.
    pub velocity_error_px_s: Option<f64>,
    /// The first scroll event to the first presented frame whose content moved.
    pub start_latency_ms: Option<f64>,
    /// The evenness of the per-refresh displacement, 0 to 100.
    pub smoothness_score: Option<f64>,
    /// The rejected steps by reason.
    pub rejections: BTreeMap<String, usize>,
    /// Every step in order.
    pub steps: Vec<FrameStep>,
}

impl Metrics for ScrollReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.higher("smoothness_score", self.smoothness_score, Unit::Score);
        m.lower("stalls", Some(self.stalls as f64), Unit::Count);
        m.lower(
            "missed_vblanks",
            Some(self.missed_vblanks as f64),
            Unit::Count,
        );
        m.count("stalled_frames", self.stalled_frames);
        m.count("double_steps", self.double_steps);
        m.lower("relative_error", self.relative_error, Unit::Fraction);
        m.lower(
            "step_error_abs_max",
            self.step_error_px.map(|s| s.max.abs().max(s.min.abs())),
            Unit::Px,
        );
        m.neutral("tracking_ratio", self.tracking_ratio, Unit::Ratio);
        m.neutral("velocity_p50", self.velocity_px_s.p50(), Unit::PxPerS);
        m.lower("velocity_error", self.velocity_error_px_s, Unit::PxPerS);
        m.lower("start_latency", self.start_latency_ms, Unit::Ms);
        m.count("rejected_steps", self.rejected_steps);
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        Some(self.active_steps.saturating_sub(self.rejected_steps))
    }
}
