//! The startup report.

use fakeland_types::{ProcessId, Timestamp, WindowId};
use serde::{Deserialize, Serialize};

use crate::metrics::{Metric, MetricList, Metrics, Unit};

/// The startup milestones of one app launch. Every `*_ms` value counts from the spawn.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StartupReport {
    /// The process.
    pub pid: Option<ProcessId>,
    /// The command line.
    pub command: Option<String>,
    /// The spawn time.
    pub spawned: Option<Timestamp>,
    /// The window.
    pub window: Option<WindowId>,
    /// The first Wayland connection of the app.
    pub connected_ms: Option<f64>,
    /// The first surface of the app.
    pub first_surface_ms: Option<f64>,
    /// The first mapped toplevel.
    pub toplevel_ms: Option<f64>,
    /// The first commit with a buffer in the window tree.
    pub first_buffer_ms: Option<f64>,
    /// The first modeled presentation of the window tree.
    pub first_present_ms: Option<f64>,
    /// The time the window content became stable.
    pub visually_complete_ms: Option<f64>,
    /// The presentation of the response to the first probe input.
    pub responsive_ms: Option<f64>,
    /// The latency of that first probe input.
    pub first_input_latency_ms: Option<f64>,
    /// The app exited before it mapped a window.
    pub exited_early: bool,
}

impl Metrics for StartupReport {
    fn metrics(&self) -> Vec<Metric> {
        let mut m = MetricList::new();
        m.lower("connected", self.connected_ms, Unit::Ms);
        m.lower("first_surface", self.first_surface_ms, Unit::Ms);
        m.lower("toplevel", self.toplevel_ms, Unit::Ms);
        m.lower("first_buffer", self.first_buffer_ms, Unit::Ms);
        m.lower("first_present", self.first_present_ms, Unit::Ms);
        m.lower("visually_complete", self.visually_complete_ms, Unit::Ms);
        m.lower("responsive", self.responsive_ms, Unit::Ms);
        m.lower("first_input_latency", self.first_input_latency_ms, Unit::Ms);
        m.into_vec()
    }
}
