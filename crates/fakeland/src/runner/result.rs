//! The result of a scenario run and its parts.

use fakeland_trace::Trace;
use fakeland_types::{Nanos, RunId, Timestamp};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::RunErrorKind;
use crate::capture::Screenshot;
use crate::framecap::{CaptureSpec, FrameCapture};
use crate::probes::ProbeReport;

/// Options of a run beyond the scenario.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct RunOptions {
    /// Frame captures that run for the whole scenario.
    pub frames: Vec<CaptureSpec>,
}

/// The timing of one executed step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StepTiming {
    /// The position in execution order, from 0.
    pub index: u32,
    /// The path in the scenario, such as `steps[1].steps[0]`.
    pub path: String,
    /// The iteration of the innermost repeat block.
    pub iteration: Option<u32>,
    /// The step label.
    pub label: Option<String>,
    /// The action type.
    pub action: String,
    /// The time the anchor named; equal to `started` for steps that start at once.
    pub scheduled: Timestamp,
    /// The time the action started.
    pub started: Timestamp,
    /// The time the action finished.
    pub finished: Timestamp,
    /// The count of injected events that reached a client.
    pub inputs: usize,
}

impl StepTiming {
    /// Returns the start lateness against the anchor.
    pub fn lateness(&self) -> Nanos {
        self.started.since(self.scheduled)
    }
}

/// One firing of a trigger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TriggerFiring {
    /// The trigger index in the scenario.
    pub trigger: usize,
    /// The trigger label.
    pub label: Option<String>,
    /// The record type that fired it.
    pub event: String,
    /// The time of that record.
    pub event_time: Timestamp,
    /// The time the actions started.
    pub fired: Timestamp,
    /// The firing count of the trigger so far, from 1.
    pub count: u32,
    /// The error of the first failing action.
    pub error: Option<String>,
}

/// A screenshot that a capture step took.
#[derive(Clone, Debug, PartialEq)]
pub struct CaptureResult {
    /// The step path.
    pub path: String,
    /// The capture label.
    pub label: Option<String>,
    /// The screenshot.
    pub screenshot: Screenshot,
}

/// The result of a scenario run. Every time is a compositor timestamp.
#[derive(Clone, Debug)]
pub struct RunResult {
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
    /// The executed steps in order.
    pub steps: Vec<StepTiming>,
    /// The probe reports in step order.
    pub probes: Vec<ProbeReport>,
    /// Every trigger firing.
    pub triggers: Vec<TriggerFiring>,
    /// The screenshots of capture steps.
    pub captures: Vec<CaptureResult>,
    /// The frame captures of the run options.
    pub frames: Vec<FrameCapture>,
    /// The trace of the run span with its live context.
    pub trace: Trace,
}

impl RunResult {
    /// Returns the duration of the run.
    pub fn duration(&self) -> Nanos {
        self.finished.since(self.started)
    }
}
