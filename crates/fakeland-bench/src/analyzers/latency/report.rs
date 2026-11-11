//! The latency report.

use fakeland_trace::Summary;
use fakeland_trace::stats::{ConfidenceInterval, Histogram};
use serde::{Deserialize, Serialize};

use super::PhaseBin;
use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// One probe repeat in compact form.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LatencySample {
    /// The probe repeat index.
    pub repeat: u32,
    /// The phase of the input send within its refresh period.
    pub phase_ms: Option<f64>,
    /// Input send to commit of the detected frame.
    pub input_to_commit_ms: Option<f64>,
    /// Input send to modeled presentation.
    pub total_ms: Option<f64>,
    /// Nothing matched before the timeout.
    pub timed_out: bool,
}

/// Input latency over probe repeats.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LatencyReport {
    /// The probe label the analyzer selected.
    pub label: Option<String>,
    /// The probe repeats.
    pub repeats: usize,
    /// Repeats whose detector matched.
    pub detected: usize,
    /// Repeats whose response was presented.
    pub presented: usize,
    /// Repeats without a match before the timeout.
    pub timed_out: usize,
    /// Input send to modeled presentation.
    pub total_ms: Option<Summary>,
    /// Input send to the commit of the detected frame.
    pub input_to_commit_ms: Option<Summary>,
    /// Commit to fence ready.
    pub commit_to_ready_ms: Option<Summary>,
    /// Fence ready to latch.
    pub ready_to_latch_ms: Option<Summary>,
    /// Latch to modeled presentation.
    pub latch_to_present_ms: Option<Summary>,
    /// The 95% bootstrap interval of the median total.
    pub total_median_ci: Option<ConfidenceInterval>,
    /// The 95% bootstrap interval of the median input-to-commit time.
    pub input_to_commit_median_ci: Option<ConfidenceInterval>,
    /// The totals by the phase of the input in its refresh period.
    pub by_phase: Vec<PhaseBin>,
    /// The totals in 1 ms bins.
    pub histogram: Option<Histogram>,
    /// The compositor time spent on readbacks per repeat.
    pub readback_cost_ms: Option<Summary>,
    /// Every repeat.
    pub samples: Vec<LatencySample>,
}

impl Metrics for LatencyReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.lower("total_p50", self.total_ms.p50(), Unit::Ms);
        m.lower("total_p95", self.total_ms.p95(), Unit::Ms);
        m.lower("total_mean", self.total_ms.mean(), Unit::Ms);
        m.lower("total_max", self.total_ms.max(), Unit::Ms);
        m.lower(
            "input_to_commit_p50",
            self.input_to_commit_ms.p50(),
            Unit::Ms,
        );
        m.lower(
            "input_to_commit_p95",
            self.input_to_commit_ms.p95(),
            Unit::Ms,
        );
        m.lower(
            "commit_to_ready_p50",
            self.commit_to_ready_ms.p50(),
            Unit::Ms,
        );
        m.lower("ready_to_latch_p50", self.ready_to_latch_ms.p50(), Unit::Ms);
        m.lower(
            "latch_to_present_p50",
            self.latch_to_present_ms.p50(),
            Unit::Ms,
        );
        m.count("timeouts", self.timed_out);
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        Some(self.total_ms.n())
    }
}
