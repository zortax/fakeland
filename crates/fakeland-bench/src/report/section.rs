//! One analyzer section of a bench report.

use serde::{Deserialize, Serialize};

use crate::metrics::{Metric, MetricSummary};

/// The report of one analyzer for one iteration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct IterationReport {
    /// The iteration index over the whole bench.
    pub index: u32,
    /// The variant label.
    #[serde(default)]
    pub variant: Option<String>,
    /// The typed report as JSON.
    pub report: serde_json::Value,
}

/// The report of one analyzer over the pooled samples of one variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PooledReport {
    /// The variant label.
    #[serde(default)]
    pub variant: Option<String>,
    /// The count of pooled samples.
    #[serde(default)]
    pub samples: Option<usize>,
    /// The metrics of the pooled report.
    pub metrics: Vec<Metric>,
    /// The typed report as JSON.
    pub report: serde_json::Value,
}

/// The results of one analyzer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Section {
    /// The section name, such as `frame_pacing`.
    pub name: String,
    /// The metrics over the measured iterations, per variant.
    pub metrics: Vec<MetricSummary>,
    /// The count of samples behind the section, summed over iterations.
    #[serde(default)]
    pub samples: Option<usize>,
    /// The pooled reports, one per variant, for analyzers that pool.
    #[serde(default)]
    pub pooled: Vec<PooledReport>,
    /// The report of each measured iteration.
    #[serde(default)]
    pub iterations: Vec<IterationReport>,
    /// The analyzer failed.
    #[serde(default)]
    pub error: Option<String>,
}

impl Section {
    /// Builds the section of an analyzer that panicked.
    pub fn failed(name: String) -> Self {
        Self {
            name,
            metrics: Vec::new(),
            samples: None,
            pooled: Vec::new(),
            iterations: Vec::new(),
            error: Some("the analyzer panicked".into()),
        }
    }

    /// Returns the summary of a metric for a variant.
    pub fn metric(&self, name: &str, variant: Option<&str>) -> Option<&MetricSummary> {
        self.metrics
            .iter()
            .find(|m| m.name == name && m.variant.as_deref() == variant)
    }

    /// Returns the pooled metric of a variant.
    pub fn pooled_metric(&self, name: &str, variant: Option<&str>) -> Option<&Metric> {
        self.pooled
            .iter()
            .find(|p| p.variant.as_deref() == variant)
            .and_then(|p| p.metrics.iter().find(|m| m.name == name))
    }
}
