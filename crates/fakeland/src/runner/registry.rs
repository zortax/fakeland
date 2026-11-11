//! The scenario runs of a session and the state of each run.

use std::collections::HashMap;

use fakeland_trace::CaptureId;
use fakeland_types::{RunId, Timestamp};
use futures_channel::oneshot;

use super::{CaptureResult, Cursor, NextStep, RunResult, StepTiming, TriggerFiring};
use crate::actions::{ActionError, ActionOutcome};
use crate::display::TimerId;
use crate::framecap::FrameCaptureId;
use crate::probes::ProbeReport;
use crate::triggers::TriggerSet;
use crate::util::CancelToken;
use crate::waits::ObserverId;

/// The step that runs now or waits for its anchor.
#[derive(Debug, Clone)]
pub struct CurrentStep {
    /// The step and its path.
    pub next: NextStep,
    /// The position in execution order.
    pub index: u32,
    /// The time the anchor named.
    pub scheduled: Timestamp,
    /// The start time of the action.
    pub started: Option<Timestamp>,
}

/// The result of an action that finished while its step was starting.
pub type SyncDone = Option<Result<ActionOutcome, ActionError>>;

/// One scenario run.
pub struct Run {
    /// The scenario name.
    pub name: String,
    /// The start time.
    pub started: Timestamp,
    /// The trace capture of the run span.
    pub capture: CaptureId,
    /// The step cursor.
    pub cursor: Cursor,
    /// Stops every action, wait and trigger of the run.
    pub token: CancelToken,
    /// The generation of the current step; callbacks of older steps do nothing.
    pub generation: u64,
    /// The current step.
    pub current: Option<CurrentStep>,
    /// The end time of the previous step.
    pub prev_finished: Timestamp,
    /// The count of steps started.
    pub index: u32,
    /// The finished steps.
    pub steps: Vec<StepTiming>,
    /// The probe reports.
    pub probes: Vec<ProbeReport>,
    /// The screenshots of capture steps.
    pub captures: Vec<CaptureResult>,
    /// The armed triggers.
    pub triggers: Option<TriggerSet>,
    /// The trigger observer.
    pub trigger_observer: Option<ObserverId>,
    /// Every trigger firing.
    pub trigger_log: Vec<TriggerFiring>,
    /// The frame captures of the run options.
    pub frames: Vec<FrameCaptureId>,
    /// The scenario deadline.
    pub timeout: Option<TimerId>,
    /// The caller waiting for the result.
    pub reply: Option<oneshot::Sender<RunResult>>,
    /// True while a step starts its action.
    pub in_begin: bool,
    /// The outcome of an action that finished while its step was starting.
    pub sync_done: SyncDone,
}

/// Every active run.
#[derive(Default)]
pub struct Runs {
    /// Runs by id.
    pub active: HashMap<RunId, Run>,
}

impl std::fmt::Debug for Runs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runs")
            .field("active", &self.active.len())
            .finish()
    }
}
