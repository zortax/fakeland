//! Parameters of the scenario, measurement, statistics, trace and benchmark tools.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::Region;
use super::defaults;

/// A scenario to run.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RunScenarioParams {
    /// The scenario as JSON. Call `scenario_schema` for its full schema. Example:
    /// `{"name": "demo", "steps": [{"action": {"type": "pointer", "op": "click", "at":
    /// {"type": "window_fraction", "fx": 0.5, "fy": 0.5}}}, {"action": {"type": "wait",
    /// "until": "settled"}}, {"action": {"type": "capture"}}]}`
    pub scenario: Value,
    /// Wait for the end. Without it, the tool returns the run id at once.
    #[serde(default = "defaults::yes")]
    pub wait: bool,
    /// The longest wait in milliseconds.
    #[serde(default = "defaults::scenario_ms")]
    pub timeout_ms: u64,
    /// Write the trace of the run to this absolute path (.jsonl, .json, .perfetto.json, .csv).
    #[serde(default)]
    pub trace_path: Option<String>,
    /// Write capture steps as PNG files into this absolute directory.
    #[serde(default)]
    pub captures_dir: Option<String>,
}

/// The input of a simple latency measurement.
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LatencyAction {
    /// Move the pointer to (x, y); odd repeats move to `away`.
    Hover,
    /// Click at (x, y).
    Click,
    /// Press `key`.
    Key,
}

/// What detects the response.
#[derive(Clone, Copy, Debug, Default, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LatencyDetector {
    /// The pixels of `roi` change.
    #[default]
    PixelsChanged,
    /// The app reports damage in `roi`.
    AnyDamage,
    /// The app marks the input as handled (needs app marks).
    AppMark,
}

/// A latency measurement: a full probe spec, or the simple form.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MeasureLatencyParams {
    /// A full probe spec as JSON. It replaces the simple form.
    #[serde(default)]
    pub probe: Option<Value>,
    /// The simple form: the input.
    #[serde(default)]
    pub action: Option<LatencyAction>,
    /// The input position.
    #[serde(default)]
    pub x: f64,
    /// The input position.
    #[serde(default)]
    pub y: f64,
    /// The position for odd hover repeats, as `[x, y]`. It makes the hover toggle.
    #[serde(default)]
    pub away: Option<[f64; 2]>,
    /// The key of the `key` action.
    #[serde(default)]
    pub key: Option<String>,
    /// The coordinates are logical output coordinates, not screenshot pixels.
    #[serde(default)]
    pub logical: bool,
    /// The coordinates and `roi` are logical and relative to this window.
    #[serde(default)]
    pub window: Option<String>,
    /// The region that must respond, logical, relative to `window` or to the output.
    #[serde(default)]
    pub roi: Option<Region>,
    /// The detector.
    #[serde(default)]
    pub detector: LatencyDetector,
    /// The count of repeats.
    #[serde(default = "defaults::repeat")]
    pub repeat: u32,
}

/// A request for frame statistics.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct FrameStatsParams {
    /// The window.
    #[serde(default = "defaults::focused")]
    pub window: String,
    /// The span before now in milliseconds.
    #[serde(default = "defaults::stats_ms")]
    pub last_ms: u64,
}

/// No parameters.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct TraceStartParams {}

/// Where the trace goes.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TraceStopParams {
    /// The absolute file path. The name picks the format: .jsonl, .json, .perfetto.json (open in
    /// ui.perfetto.dev), .csv or .summary.json.
    pub path: String,
}

/// A benchmark preset run.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BenchmarkParams {
    /// The preset name, such as `pacing` or `hover-latency`. `list` lists every preset with
    /// its parameters and defaults.
    pub preset: String,
    /// Preset parameters as a JSON object. Missing parameters take their defaults.
    #[serde(default)]
    pub params: Value,
    /// The measured iterations.
    #[serde(default)]
    pub iterations: Option<u32>,
    /// The window under test: focused (default), any, an id, `app:APP_ID` or `title:REGEX`.
    #[serde(default)]
    pub window: Option<String>,
    /// Write a Chrome trace of the whole bench to this absolute path.
    #[serde(default)]
    pub perfetto_path: Option<String>,
    /// Write the report JSON to this absolute path.
    #[serde(default)]
    pub report_path: Option<String>,
}
