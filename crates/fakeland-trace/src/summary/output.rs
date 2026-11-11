//! The summary of one output.

use fakeland_types::OutputId;
use serde::{Deserialize, Serialize};

use crate::frames::FrameTimeline;
use crate::stats::Summary;

/// Vblank counts and compositor wake lateness of one output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OutputSummary {
    /// The output.
    pub output: OutputId,
    /// The connector name.
    pub name: String,
    /// The refresh rate in hertz.
    pub refresh_hz: Option<f64>,
    /// The number of vblanks.
    pub vblanks: usize,
    /// The number of vblanks the perturbation model drops.
    pub dropped_vblanks: usize,
    /// The compositor wake lateness in microseconds.
    pub wake_lateness_us: Option<Summary>,
    /// The number of frames presented on the output, all surfaces together.
    pub presented_frames: usize,
}

impl OutputSummary {
    /// Summarizes one output of a timeline.
    pub fn build(tl: &FrameTimeline, output: OutputId) -> Self {
        let info = tl.output(output);
        let vblanks = tl.vblanks(output);
        let lateness: Vec<f64> = vblanks
            .iter()
            .map(|v| v.lateness().as_micros_f64())
            .collect();
        Self {
            output,
            name: info.map(|o| o.name.clone()).unwrap_or_default(),
            refresh_hz: info.and_then(|o| o.refresh()).map(|h| h.0),
            vblanks: vblanks.len(),
            dropped_vblanks: vblanks.iter().filter(|v| v.dropped).count(),
            wake_lateness_us: Summary::of(&lateness),
            presented_frames: tl.presented_on(output).len(),
        }
    }
}
