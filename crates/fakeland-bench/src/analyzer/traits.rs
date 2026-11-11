//! The analyzer trait: one iteration in, one typed report out.

use serde::Serialize;

use crate::input::AnalysisInput;
use crate::metrics::Metrics;

/// Turns the trace of one iteration into a typed report. An analyzer reads data only and never
/// talks to the compositor.
pub trait Analyzer: Send + Sync + 'static {
    /// The report of one iteration.
    type Report: Serialize + Metrics + Send + 'static;

    /// Returns the section name in bench reports, such as `frame_pacing`.
    fn name(&self) -> String;

    /// Analyzes one iteration.
    fn analyze(&self, input: &AnalysisInput) -> Self::Report;

    /// Analyzes the samples of several iterations of one variant together. Analyzers whose
    /// statistics need many samples, such as tail latencies, pool them here. The default pools
    /// nothing.
    fn pool(&self, inputs: &[&AnalysisInput]) -> Option<Self::Report> {
        let _ = inputs;
        None
    }
}
