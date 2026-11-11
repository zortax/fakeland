//! Baseline comparison: each metric of a report against the same metric of a baseline, with
//! thresholds and a regression verdict per metric.

mod comparison;
mod pattern;
mod text;
mod thresholds;

#[cfg(test)]
mod tests;

pub use comparison::{Comparison, MetricComparison, Outcome, Verdict, compare};
pub use pattern::matches;
pub use thresholds::{MetricThreshold, Thresholds};
