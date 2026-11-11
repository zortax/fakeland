//! The input coalescing report.

use std::collections::BTreeMap;

use fakeland_trace::Summary;
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// Input events per presented frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CoalescingReport {
    /// Actionable input events.
    pub inputs: usize,
    /// Input events whose response was presented.
    pub presented_inputs: usize,
    /// Presented frames that answer at least one input.
    pub frames: usize,
    /// Inputs per answering frame.
    pub per_frame: Option<Summary>,
    /// The largest count of inputs one frame answers.
    pub max_per_frame: usize,
    /// Input events per second from the first to the last send.
    pub input_rate_hz: Option<f64>,
    /// Answering frames per second over the same span.
    pub frame_rate_hz: Option<f64>,
    /// Inputs per kind.
    pub by_kind: BTreeMap<String, usize>,
}

impl Metrics for CoalescingReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.neutral("inputs_per_frame_mean", self.per_frame.mean(), Unit::Count);
        m.neutral(
            "inputs_per_frame_max",
            Some(self.max_per_frame as f64),
            Unit::Count,
        );
        m.neutral("input_rate", self.input_rate_hz, Unit::Count);
        m.count("unpresented_inputs", self.inputs - self.presented_inputs);
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        (self.inputs > 0).then_some(self.frames)
    }
}
