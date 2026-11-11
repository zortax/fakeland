//! The throughput report.

use fakeland_trace::Summary;
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// The frame rate a client reaches and the GPU time of its frames.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ThroughputReport {
    /// Commits with a buffer.
    pub commits: usize,
    /// Presented frames.
    pub presented: usize,
    /// Discarded commits.
    pub discarded: usize,
    /// The span from the first to the last buffer commit.
    pub duration_ms: f64,
    /// Buffer commits per second.
    pub commit_fps: Option<f64>,
    /// Presented frames per second over the same span.
    pub present_fps: Option<f64>,
    /// The time between buffer commits.
    pub commit_interval_ms: Option<Summary>,
    /// Commit to fence ready, for frames with a fence.
    pub gpu_ms: Option<Summary>,
    /// Fence ready to modeled presentation, never negative.
    pub ready_to_present_ms: Option<Summary>,
    /// Frames with explicit sync over buffer commits.
    pub explicit_sync_fraction: f64,
    /// Presentations carry the vsync flag, so the output throttles the client.
    pub vsync: bool,
}

impl Metrics for ThroughputReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.higher("commit_fps", self.commit_fps, Unit::Fps);
        m.higher("present_fps", self.present_fps, Unit::Fps);
        m.lower(
            "commit_interval_p50",
            self.commit_interval_ms.p50(),
            Unit::Ms,
        );
        m.lower(
            "commit_interval_p99",
            self.commit_interval_ms.p99(),
            Unit::Ms,
        );
        m.lower("gpu_p50", self.gpu_ms.p50(), Unit::Ms);
        m.lower("gpu_p95", self.gpu_ms.p95(), Unit::Ms);
        m.lower(
            "ready_to_present_p50",
            self.ready_to_present_ms.p50(),
            Unit::Ms,
        );
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        Some(self.commits)
    }
}
