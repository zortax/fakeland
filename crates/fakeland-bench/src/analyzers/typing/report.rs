//! The typing report.

use fakeland_trace::Summary;
use fakeland_types::{CommitId, InputEventId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// One glyph key press or text event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyLatency {
    /// The input event.
    pub input: InputEventId,
    /// The key name, or the committed text.
    pub key: String,
    /// The send time.
    pub sent: Timestamp,
    /// The commit that handled it.
    pub handled_by: Option<CommitId>,
    /// Send to the handling commit.
    pub handling_ms: Option<f64>,
    /// Send to the modeled presentation of the response.
    pub latency_ms: Option<f64>,
    /// Earlier glyph keys not yet handled when this one was sent.
    pub backlog: usize,
}

/// Key-to-glyph latency over a typing run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypingReport {
    /// Glyph key presses and text events.
    pub keys: usize,
    /// Keys with a handling commit.
    pub handled: usize,
    /// Keys whose response was presented.
    pub presented: usize,
    /// Send to the modeled presentation.
    pub latency_ms: Option<Summary>,
    /// Send to the handling commit.
    pub handling_ms: Option<Summary>,
    /// The largest backlog.
    pub max_backlog: usize,
    /// The mean backlog.
    pub mean_backlog: f64,
    /// The least-squares slope of latency over key index; a growing backlog makes it positive.
    pub latency_slope_ms_per_key: Option<f64>,
    /// Keys per second from the first to the last send.
    pub keys_per_s: Option<f64>,
    /// Every key in send order.
    pub per_key: Vec<KeyLatency>,
}

impl Metrics for TypingReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.lower("latency_p50", self.latency_ms.p50(), Unit::Ms);
        m.lower("latency_p95", self.latency_ms.p95(), Unit::Ms);
        m.lower("latency_max", self.latency_ms.max(), Unit::Ms);
        m.lower("handling_p50", self.handling_ms.p50(), Unit::Ms);
        m.count("max_backlog", self.max_backlog);
        m.lower("latency_slope", self.latency_slope_ms_per_key, Unit::Ms);
        m.count("unpresented", self.keys - self.presented);
        m.neutral("keys_per_s", self.keys_per_s, Unit::Count);
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        Some(self.presented)
    }
}
