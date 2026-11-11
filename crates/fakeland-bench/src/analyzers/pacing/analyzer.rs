//! The frame pacing analyzer.

use fakeland_types::SurfaceId;

use super::build::build;
use super::{FramePacingReport, PacingSamples};
use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;

/// Measures the frame pacing of the target's main surface.
#[derive(Clone, Debug)]
pub struct FramePacing {
    /// A frame is jank when its present interval is longer than this factor times the median
    /// interval.
    pub jank_factor: f64,
    /// The surface to measure; `None` takes the main surface of the target.
    pub surface: Option<SurfaceId>,
}

impl Default for FramePacing {
    fn default() -> Self {
        Self {
            jank_factor: 1.5,
            surface: None,
        }
    }
}

impl FramePacing {
    /// Returns the analyzer with another jank factor.
    pub fn jank_factor(mut self, factor: f64) -> Self {
        self.jank_factor = factor;
        self
    }
}

impl Analyzer for FramePacing {
    type Report = FramePacingReport;

    fn name(&self) -> String {
        "frame_pacing".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> FramePacingReport {
        build(
            &PacingSamples::extract(input, self.surface),
            self.jank_factor,
            true,
        )
    }

    fn pool(&self, inputs: &[&AnalysisInput]) -> Option<FramePacingReport> {
        if inputs.len() < 2 {
            return None;
        }
        let mut all = PacingSamples::default();
        for input in inputs {
            all.merge(PacingSamples::extract(input, self.surface));
        }
        Some(build(&all, self.jank_factor, false))
    }
}
