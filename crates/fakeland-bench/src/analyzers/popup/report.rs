//! The popup open report.

use fakeland_trace::Summary;
use fakeland_types::{InputEventId, SurfaceId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::analyzers::util::SampleStats;
use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// One popup or new window and the input that opened it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PopupRecord {
    /// The surface.
    pub surface: SurfaceId,
    /// The role: `xdg_popup` or `xdg_toplevel` for a new window.
    pub role: String,
    /// The input that opened it: the last press before the popup got its role.
    pub input: Option<InputEventId>,
    /// The send time of that input.
    pub input_sent: Option<Timestamp>,
    /// The time the surface got its role.
    pub mapped: Timestamp,
    /// Input to role.
    pub map_ms: Option<f64>,
    /// Input to the first commit with a buffer.
    pub first_buffer_ms: Option<f64>,
    /// Input to the first modeled presentation.
    pub first_present_ms: Option<f64>,
}

/// How fast popups open.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PopupReport {
    /// The popups that an input opened.
    pub opened: usize,
    /// Press inputs that opened nothing.
    pub unanswered_presses: usize,
    /// Input to role.
    pub map_ms: Option<Summary>,
    /// Input to first buffer.
    pub first_buffer_ms: Option<Summary>,
    /// Input to first presentation.
    pub first_present_ms: Option<Summary>,
    /// Every popup in order.
    pub popups: Vec<PopupRecord>,
}

impl Metrics for PopupReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.lower("map_p50", self.map_ms.p50(), Unit::Ms);
        m.lower("first_buffer_p50", self.first_buffer_ms.p50(), Unit::Ms);
        m.lower("first_present_p50", self.first_present_ms.p50(), Unit::Ms);
        m.lower("first_present_max", self.first_present_ms.max(), Unit::Ms);
        m.neutral("opened", Some(self.opened as f64), Unit::Count);
        m.count("unanswered_presses", self.unanswered_presses);
        m.into_vec()
    }

    fn samples(&self) -> Option<usize> {
        Some(self.first_present_ms.n())
    }
}
