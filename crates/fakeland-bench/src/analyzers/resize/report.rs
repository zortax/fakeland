//! The resize report.

use fakeland_trace::Summary;
use serde::{Deserialize, Serialize};

use super::ConfigureRecord;
use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// How a window answered configures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResizeReport {
    /// The configures of the toplevel.
    pub configures: usize,
    /// Configures the client acked.
    pub acked: usize,
    /// Configures the client skipped for a newer one.
    pub superseded: usize,
    /// Acked configures with a size whose answering commit has that size, after the margin.
    pub matched: usize,
    /// Configure to its ack.
    pub configure_to_ack_ms: Option<Summary>,
    /// Configure to the first commit that answers it.
    pub configure_to_commit_ms: Option<Summary>,
    /// Configure to the presentation of that commit.
    pub configure_to_present_ms: Option<Summary>,
    /// The typical logical size difference between buffers and configures, such as client-side
    /// shadows.
    pub margin: (f64, f64),
    /// Presented frames whose size differs from the configure they answer, after the margin.
    pub mismatched_frames: usize,
    /// Refreshes without a new frame inside configure storms: runs of configures less than
    /// 100 ms apart, from the first send to the last presented answer.
    pub missed_vblanks: u64,
    /// Commits discarded inside storms.
    pub discarded: usize,
    /// The longest storm.
    pub storm_ms: Option<f64>,
    /// Every configure in order.
    pub records: Vec<ConfigureRecord>,
}

impl Metrics for ResizeReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.lower(
            "configure_to_ack_p50",
            self.configure_to_ack_ms.p50(),
            Unit::Ms,
        );
        m.lower(
            "configure_to_commit_p50",
            self.configure_to_commit_ms.p50(),
            Unit::Ms,
        );
        m.lower(
            "configure_to_commit_p95",
            self.configure_to_commit_ms.p95(),
            Unit::Ms,
        );
        m.lower(
            "configure_to_present_p50",
            self.configure_to_present_ms.p50(),
            Unit::Ms,
        );
        m.lower(
            "configure_to_present_max",
            self.configure_to_present_ms.max(),
            Unit::Ms,
        );
        m.count("mismatched_frames", self.mismatched_frames);
        m.lower(
            "missed_vblanks",
            Some(self.missed_vblanks as f64),
            Unit::Count,
        );
        m.count("discarded", self.discarded);
        m.neutral("superseded", Some(self.superseded as f64), Unit::Count);
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        Some(self.configure_to_commit_ms.n())
    }
}
