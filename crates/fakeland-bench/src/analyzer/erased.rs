//! The object-safe form of [`Analyzer`] that the runner stores, and the section it builds.

use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;
use crate::metrics::{Metrics, aggregate};
use crate::report::{IterationReport, PooledReport, Section};

/// An analyzer behind a trait object. Every [`Analyzer`] implements it.
pub trait DynAnalyzer: Send + Sync {
    /// Returns the section name.
    fn section_name(&self) -> String;

    /// Analyzes the measured iterations and builds the report section. `seed` seeds the
    /// bootstrap intervals.
    fn section(&self, inputs: &[&AnalysisInput], seed: u64) -> Section;
}

impl<A: Analyzer> DynAnalyzer for A {
    fn section_name(&self) -> String {
        self.name()
    }

    fn section(&self, inputs: &[&AnalysisInput], seed: u64) -> Section {
        let mut per = Vec::with_capacity(inputs.len());
        let mut iterations = Vec::with_capacity(inputs.len());
        let mut samples: Option<usize> = None;
        for input in inputs {
            let report = self.analyze(input);
            let variant = input.iteration.variant.clone();
            if let Some(n) = report.samples() {
                *samples.get_or_insert(0) += n;
            }
            per.push((variant.clone(), report.metrics()));
            iterations.push(IterationReport {
                index: input.iteration.index,
                variant,
                report: to_value(&report),
            });
        }
        let mut variants: Vec<Option<String>> = Vec::new();
        for input in inputs {
            if !variants.contains(&input.iteration.variant) {
                variants.push(input.iteration.variant.clone());
            }
        }
        let pooled = variants
            .into_iter()
            .filter_map(|variant| {
                let group: Vec<&AnalysisInput> = inputs
                    .iter()
                    .copied()
                    .filter(|i| i.iteration.variant == variant)
                    .collect();
                let report = self.pool(&group)?;
                Some(PooledReport {
                    variant,
                    samples: report.samples(),
                    metrics: report.metrics(),
                    report: to_value(&report),
                })
            })
            .collect();
        Section {
            name: self.name(),
            metrics: aggregate(&per, seed),
            samples,
            pooled,
            iterations,
            error: None,
        }
    }
}

fn to_value<T: serde::Serialize>(value: &T) -> serde_json::Value {
    serde_json::to_value(value)
        .unwrap_or_else(|e| serde_json::json!({ "serialize_error": e.to_string() }))
}
