//! The frame cost report.

use fakeland_trace::Summary;
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// What the frames of the target's main surface cost over an iteration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameCostReport {
    /// Presented content frames.
    pub frames: usize,
    /// Content commits.
    pub commits: usize,
    /// The iteration span.
    pub span_ms: f64,
    /// The time on CPU of the trees under test from their first to their last sample.
    pub cpu_ms: Option<f64>,
    /// The presented frames inside the CPU sample window.
    pub cpu_frames: usize,
    /// The time on CPU per presented frame inside the CPU sample window.
    pub cpu_ms_per_frame: Option<f64>,
    /// Commit to fence ready, for content commits with a fence.
    pub gpu_fence_ms: Option<Summary>,
    /// The fence time of every content commit per second of the iteration.
    pub gpu_fence_ms_per_s: Option<f64>,
    /// The damaged area over the buffer area of each commit with a buffer, clipped to 1.
    pub damage_fraction: Option<Summary>,
    /// Commits with a buffer whose damage covers more than the full damage fraction.
    pub full_damage_frames: usize,
}

impl Metrics for FrameCostReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.neutral("frames", Some(self.frames as f64), Unit::Count);
        m.lower("cpu_ms_per_frame", self.cpu_ms_per_frame, Unit::Ms);
        m.lower("gpu_fence_ms_p50", self.gpu_fence_ms.p50(), Unit::Ms);
        m.lower("gpu_fence_ms_p95", self.gpu_fence_ms.p95(), Unit::Ms);
        m.lower("gpu_fence_ms_per_s", self.gpu_fence_ms_per_s, Unit::MsPerS);
        m.lower(
            "damage_fraction_p50",
            self.damage_fraction.p50(),
            Unit::Fraction,
        );
        m.lower(
            "damage_fraction_mean",
            self.damage_fraction.mean(),
            Unit::Fraction,
        );
        if self.damage_fraction.is_some() {
            m.count("full_damage_frames", self.full_damage_frames);
        }
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        (self.commits > 0).then_some(self.commits)
    }
}
