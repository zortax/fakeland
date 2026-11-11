//! Benchmark runner and analyzers. A [`Bench`] repeats a driver (a scenario, a reactor or custom
//! async code) over a [`fakeland::Session`] with warmup, settle times and phase sweeps, and
//! captures a trace per iteration. Each [`Analyzer`] turns one iteration into a typed report with
//! named [`Metric`]s. The runner aggregates the metrics over iterations with bootstrap confidence
//! intervals into a [`BenchReport`], and [`compare()`] checks a report against a baseline.
//! [`presets`] holds ready benches for common questions.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod analyzer;
pub mod analyzers;
pub mod compare;
mod error;
pub mod harness;
pub mod input;
pub mod metrics;
pub mod presets;
pub mod reactors;
pub mod report;
pub mod runner;
pub mod validity;

#[cfg(test)]
mod testing;

pub use analyzer::{Analyzer, AnalyzerSet, DynAnalyzer};
pub use compare::{Comparison, Thresholds, compare};
pub use error::BenchError;
pub use harness::Harness;
pub use input::{AnalysisInput, IterationMeta, Observation, RunSummary, Target};
pub use metrics::{Better, Metric, MetricList, MetricSummary, Metrics, Unit};
pub use presets::{Preset, PresetError, PresetInfo, PresetParams};
pub use report::{BenchReport, REPORT_FORMAT_VERSION, Section};
pub use runner::{
    Bench, BenchRun, Driver, DriverOutput, IterationContext, IterationRecord, Variant, driver_fn,
};
pub use validity::{Check, CheckStatus, Validity, ValidityConfig};
