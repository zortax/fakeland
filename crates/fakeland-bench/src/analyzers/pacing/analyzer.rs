//! The frame pacing analyzer.

use fakeland_types::SurfaceId;
use fakeland_vision::Axis;

use super::build::build;
use super::samples::CommitWindow;
use super::{FramePacingReport, PacingSamples};
use crate::analyzer::Analyzer;
use crate::analyzers::scroll::scroll_inputs;
use crate::input::AnalysisInput;

/// Measures the frame pacing of the target's main surface.
#[derive(Clone, Debug)]
pub struct FramePacing {
    /// A frame is jank when its present interval is longer than this factor times the median
    /// interval.
    pub jank_factor: f64,
    /// The surface to measure; `None` takes the main surface of the target.
    pub surface: Option<SurfaceId>,
    /// Counts only the commits from the first scroll input on this axis to the last commit that
    /// consumed one. Frames before the scroll and frames that only answer its end stay out.
    /// `None` counts every commit.
    pub during_scroll: Option<Axis>,
}

impl Default for FramePacing {
    fn default() -> Self {
        Self {
            jank_factor: 1.5,
            surface: None,
            during_scroll: None,
        }
    }
}

impl FramePacing {
    /// Returns the analyzer with another jank factor.
    pub fn jank_factor(mut self, factor: f64) -> Self {
        self.jank_factor = factor;
        self
    }

    /// Returns the analyzer that counts only the commits of a scroll on an axis.
    pub fn during_scroll(mut self, axis: Axis) -> Self {
        self.during_scroll = Some(axis);
        self
    }

    fn samples(&self, input: &AnalysisInput) -> PacingSamples {
        let window = self.during_scroll.and_then(|axis| {
            let surface = self.surface.or_else(|| input.main_surface());
            let inputs = scroll_inputs(&input.timeline, surface, axis);
            Some(CommitWindow {
                start: inputs.first()?.sent,
                end: inputs.iter().filter_map(|i| i.committed).max()?,
            })
        });
        PacingSamples::extract_within(input, self.surface, window)
    }
}

impl Analyzer for FramePacing {
    type Report = FramePacingReport;

    fn name(&self) -> String {
        "frame_pacing".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> FramePacingReport {
        build(&self.samples(input), self.jank_factor, true)
    }

    fn pool(&self, inputs: &[&AnalysisInput]) -> Option<FramePacingReport> {
        if inputs.len() < 2 {
            return None;
        }
        let mut all = PacingSamples::default();
        for input in inputs {
            all.merge(self.samples(input));
        }
        Some(build(&all, self.jank_factor, false))
    }
}
