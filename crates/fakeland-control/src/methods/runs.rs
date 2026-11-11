//! Scenario runs. A run starts at once and returns its id; its summary arrives as an event and
//! through [`RunResultOf`]. Every time in a summary is a compositor timestamp.

use std::path::PathBuf;

use fakeland::{CaptureSpec, ProbeReport, RunErrorKind, StepTiming, TriggerFiring};
use fakeland_scenario::Scenario;
use fakeland_types::{Dur, RunId, Timestamp};
use serde::{Deserialize, Serialize};

use super::method::{NoParams, method};
use super::{ShotInfo, TraceFileInfo, TraceOutput};

/// A scenario to run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunScenarioParams {
    /// The scenario.
    pub scenario: Scenario,
    /// Frame captures during the run.
    #[serde(default)]
    pub frames: Vec<CaptureSpec>,
    /// Writes capture steps and frame captures as PNG files into this directory.
    #[serde(default)]
    pub captures_dir: Option<PathBuf>,
    /// Writes the trace of the run here.
    #[serde(default)]
    pub trace: Option<TraceOutput>,
}

/// A started run.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RunStarted {
    /// The run id.
    pub run: RunId,
}

/// A screenshot of a capture step.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunCapture {
    /// The step path, such as `steps[2]`.
    pub step: String,
    /// The capture label.
    pub label: Option<String>,
    /// The screenshot. Its PNG is in `captures_dir`, when the run had one.
    pub shot: ShotInfo,
}

/// A frame capture of a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunFrames {
    /// What was captured.
    pub spec: CaptureSpec,
    /// The count of kept frames.
    pub frames: usize,
    /// Frames over the limit.
    pub dropped: u64,
    /// The compositor time the captures cost, in milliseconds.
    pub cost_ms: f64,
    /// The directory of the PNG files, when the run had one.
    pub dir: Option<PathBuf>,
}

/// The serializable result of a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunSummary {
    /// The run id.
    pub run: RunId,
    /// The scenario name.
    pub name: String,
    /// The run finished every step.
    pub ok: bool,
    /// The error that ended the run.
    pub error: Option<String>,
    /// The kind of that error.
    pub error_kind: Option<RunErrorKind>,
    /// The start time.
    pub started: Timestamp,
    /// The end time.
    pub finished: Timestamp,
    /// The run time in milliseconds.
    pub duration_ms: f64,
    /// The executed steps in order.
    pub steps: Vec<StepTiming>,
    /// The probe reports in step order.
    pub probes: Vec<ProbeReport>,
    /// Every trigger firing.
    pub triggers: Vec<TriggerFiring>,
    /// The screenshots of capture steps.
    pub captures: Vec<RunCapture>,
    /// The frame captures.
    pub frames: Vec<RunFrames>,
    /// The count of trace records of the run.
    pub trace_records: usize,
    /// The written trace file.
    pub trace: Option<TraceFileInfo>,
    /// Errors while writing result files.
    #[serde(default)]
    pub file_errors: Vec<String>,
}

/// The state of a run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunState {
    /// The run id.
    pub run: RunId,
    /// The scenario name.
    pub name: String,
    /// The summary once the run has finished.
    pub summary: Option<RunSummary>,
}

/// A request for a run result.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RunResultParams {
    /// The run id.
    pub run: RunId,
    /// Waits this long for the run to finish. `None` answers at once.
    #[serde(default)]
    pub wait: Option<Dur>,
}

/// A run to cancel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelRunParams {
    /// The run id.
    pub run: RunId,
}

method!(
    /// Starts a scenario run and returns its id at once.
    RunScenario, "run_scenario", RunScenarioParams => RunStarted
);
method!(
    /// Returns the state of a run, optionally after a wait for its end.
    RunResultOf, "run_result", RunResultParams => RunState
);
method!(
    /// Cancels a run. Returns false when the run had ended.
    CancelRun, "cancel_run", CancelRunParams => bool
);
method!(
    /// Returns every run the server knows, oldest first.
    Runs, "runs", NoParams => Vec<RunState>
);
