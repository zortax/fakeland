//! The latency analyzer.

use fakeland::ProbeResult;

use super::LatencyReport;
use super::build::build;
use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;

/// Measures input latency from the probe results of each iteration.
#[derive(Clone, Debug)]
pub struct Latency {
    /// Only probes with this label count; `None` takes every probe.
    pub label: Option<String>,
    /// The count of phase slices of the refresh period.
    pub phase_bins: u32,
    /// The first repeats of each probe that do not count, for example to skip a cold first
    /// response.
    pub skip_repeats: u32,
}

impl Default for Latency {
    fn default() -> Self {
        Self {
            label: None,
            phase_bins: 8,
            skip_repeats: 0,
        }
    }
}

impl Latency {
    /// Selects the probes with a label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Skips the first repeats of each probe.
    pub fn skip_repeats(mut self, n: u32) -> Self {
        self.skip_repeats = n;
        self
    }

    fn results<'a>(&self, input: &'a AnalysisInput) -> Vec<&'a ProbeResult> {
        input
            .probes
            .iter()
            .filter(|p| self.label.is_none() || p.label == self.label)
            .flat_map(|p| p.results.iter())
            .filter(|r| r.iteration >= self.skip_repeats)
            .collect()
    }
}

impl Analyzer for Latency {
    type Report = LatencyReport;

    fn name(&self) -> String {
        match &self.label {
            Some(l) => format!("latency.{l}"),
            None => "latency".into(),
        }
    }

    fn analyze(&self, input: &AnalysisInput) -> LatencyReport {
        build(
            self.label.clone(),
            &self.results(input),
            input.period(None),
            self.phase_bins,
            input.iteration.seed,
        )
    }

    fn pool(&self, inputs: &[&AnalysisInput]) -> Option<LatencyReport> {
        let results: Vec<&ProbeResult> = inputs.iter().flat_map(|i| self.results(i)).collect();
        let period = inputs.iter().find_map(|i| i.period(None));
        Some(build(
            self.label.clone(),
            &results,
            period,
            self.phase_bins,
            0,
        ))
    }
}
