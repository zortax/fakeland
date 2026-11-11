//! Benchmark methods: list the presets and run a preset or a scenario as a bench. A bench runs
//! inside the server process; its report carries compositor timestamps only.

use std::path::PathBuf;

use fakeland_bench::{BenchReport, Comparison, PresetInfo, Thresholds};
use fakeland_scenario::{Scenario, WindowRef};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::method::{NoParams, method};
use super::{TraceFileInfo, TraceOutput};

/// A bench preset with the JSON schema and defaults of its parameters.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresetEntry {
    /// The name, such as `hover-latency`.
    pub name: String,
    /// What the preset measures and what it needs from the app.
    pub description: String,
    /// The JSON schema of the parameters.
    pub params: Value,
    /// The default parameters.
    pub defaults: Value,
}

impl From<PresetInfo> for PresetEntry {
    fn from(info: PresetInfo) -> Self {
        Self {
            name: info.name,
            description: info.description,
            params: info.params.to_value(),
            defaults: info.defaults,
        }
    }
}

/// A bench to run: a preset with parameters, or a scenario with the default analyzers.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BenchParams {
    /// The preset name.
    pub preset: Option<String>,
    /// A scenario to repeat instead of a preset.
    pub scenario: Option<Scenario>,
    /// The preset parameters as a JSON object.
    pub params: Value,
    /// The measured iterations. It overrides the parameter of the same name.
    pub iterations: Option<u32>,
    /// The warmup iterations. It overrides the parameter of the same name.
    pub warmup: Option<u32>,
    /// The window under test. It overrides the `target` parameter.
    pub target: Option<WindowRef>,
    /// Write the trace of the whole bench span to these absolute paths.
    pub traces: Vec<TraceOutput>,
    /// Write the trace of each iteration as JSONL into this absolute directory.
    pub traces_dir: Option<PathBuf>,
    /// Write the report as JSON to this absolute path.
    pub report: Option<PathBuf>,
    /// Compare the report with the report in this absolute path.
    pub baseline: Option<PathBuf>,
    /// The thresholds of the comparison.
    pub thresholds: Option<Thresholds>,
}

/// The result of a bench.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchOutcome {
    /// The report.
    pub report: BenchReport,
    /// The report as text tables.
    pub text: String,
    /// The comparison with the baseline.
    pub comparison: Option<Comparison>,
    /// The comparison as a text table.
    pub comparison_text: Option<String>,
    /// The written span traces.
    pub traces: Vec<TraceFileInfo>,
    /// Every other written file.
    pub files: Vec<PathBuf>,
}

method!(
    /// Lists the bench presets with their parameters.
    ListPresets, "presets", NoParams => Vec<PresetEntry>
);
method!(
    /// Runs a bench and returns its report.
    RunBench, "bench", BenchParams => BenchOutcome
);
