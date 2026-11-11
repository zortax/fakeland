//! Regression thresholds.

use serde::{Deserialize, Serialize};

use super::pattern::matches;

/// The threshold of the metrics whose `section.metric` name matches a pattern.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MetricThreshold {
    /// A glob over `section.metric`, such as `latency.*`.
    pub pattern: String,
    /// The relative change that counts.
    pub relative: Option<f64>,
    /// The absolute change that counts, in the unit of the metric.
    pub absolute: Option<f64>,
    /// The metric never gives a verdict.
    pub ignore: bool,
}

/// When a metric change counts as a regression or an improvement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Thresholds {
    /// A change must exceed this fraction of the baseline mean.
    pub relative: f64,
    /// A change must also exceed this absolute amount.
    pub absolute: f64,
    /// With intervals on both sides, a change counts only when the intervals do not overlap.
    pub require_ci_separation: bool,
    /// Per-metric thresholds; the last match wins.
    pub overrides: Vec<MetricThreshold>,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            relative: 0.05,
            absolute: 0.0,
            require_ci_separation: true,
            overrides: Vec::new(),
        }
    }
}

impl Thresholds {
    /// Adds a per-metric threshold.
    pub fn with(mut self, t: MetricThreshold) -> Self {
        self.overrides.push(t);
        self
    }

    /// Returns `(relative, absolute, ignore)` for a `section.metric` name.
    pub fn resolve(&self, name: &str) -> (f64, f64, bool) {
        let mut out = (self.relative, self.absolute, false);
        for o in self.overrides.iter().filter(|o| matches(&o.pattern, name)) {
            out.0 = o.relative.unwrap_or(out.0);
            out.1 = o.absolute.unwrap_or(out.1);
            out.2 = o.ignore;
        }
        out
    }
}
