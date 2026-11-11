//! The probe state machine. Each repeat waits until its target is quiet, takes baselines of the
//! region, injects the input at the chosen vblank phase, and watches the commits of the target
//! until the detector matches. It then follows the matched commit to its presentation, or to the
//! next presented commit of the same surface when the scheduler discards it.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use fakeland_scenario::{Detector, ProbeSpec, Roi, Wait, WindowRef};
use fakeland_types::{CommitId, Dur, Nanos, OutputId, SurfaceId, Timestamp, WindowId};

use super::detect::Watch;
use super::guard;
use super::prepare::prepare_repeat;
use super::{ProbeReport, ProbeResult};
use crate::actions::ActionCtx;
use crate::display::TimerId;
use crate::frames::CommitHookId;
use crate::server::State;
use crate::util::CancelToken;
use crate::waits::ObserverId;

/// The callback of a probe.
pub type ProbeDone = Box<dyn FnOnce(&mut State, ProbeReport)>;

/// Where a probe looks for its response.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// The surfaces of one window tree.
    Window(WindowId),
    /// The surfaces of every window on one output.
    Output(OutputId),
    /// Any surface, for app-mark detection.
    Any,
}

pub(super) struct ProbeRun {
    pub spec: ProbeSpec,
    pub ctx: ActionCtx,
    pub report: ProbeReport,
    pub done: Option<ProbeDone>,
    pub iteration: u32,
    pub repeat: Option<Repeat>,
    /// The longest single readback of the probe so far.
    pub readback_max: Nanos,
}

pub(super) type Shared = Rc<RefCell<ProbeRun>>;

/// One repeat in flight.
pub(super) struct Repeat {
    pub result: ProbeResult,
    pub watch: Watch,
    pub token: CancelToken,
    pub hook: Option<CommitHookId>,
    pub observer: Option<ObserverId>,
    pub timer: Option<TimerId>,
    pub output: Option<OutputId>,
    pub program_done: bool,
    pub finished: bool,
}

/// Returns the trace name of a detector.
pub fn detector_name(d: &Detector) -> &'static str {
    match d {
        Detector::AnyDamage { .. } => "any_damage",
        Detector::PixelsChanged { .. } => "pixels_changed",
        Detector::PixelsMatch { .. } => "pixels_match",
        Detector::AppMark { .. } => "app_mark",
    }
}

pub(super) fn detector_roi(d: &Detector) -> Option<&Roi> {
    match d {
        Detector::AnyDamage { roi }
        | Detector::PixelsChanged { roi, .. }
        | Detector::PixelsMatch { roi, .. } => Some(roi),
        Detector::AppMark { .. } => None,
    }
}

impl State {
    /// Runs a probe. `done` gets the report after the last repeat.
    pub fn start_probe(&mut self, spec: &ProbeSpec, ctx: ActionCtx, done: ProbeDone) {
        let report = ProbeReport {
            label: spec.label.clone(),
            detector: detector_name(&spec.detector).to_string(),
            results: Vec::new(),
        };
        let run = Rc::new(RefCell::new(ProbeRun {
            spec: spec.clone(),
            ctx,
            report,
            done: Some(done),
            iteration: 0,
            repeat: None,
            readback_max: Nanos::ZERO,
        }));
        begin_repeat(self, run);
    }
}

/// Starts the next repeat with its settle wait, or ends the probe.
pub(super) fn begin_repeat(state: &mut State, run: Shared) {
    let (spec, token, iteration) = {
        let r = run.borrow();
        (r.spec.clone(), r.ctx.token.clone(), r.iteration)
    };
    if token.is_cancelled() {
        return;
    }
    if iteration >= spec.repeat {
        let (done, report) = {
            let mut r = run.borrow_mut();
            (r.done.take(), std::mem::take(&mut r.report))
        };
        if let Some(d) = done {
            d(state, report);
        }
        return;
    }
    let window = match detector_roi(&spec.detector) {
        Some(Roi::Window { window, .. }) => state.resolve_window(window).ok().flatten(),
        _ => state.resolve_window(&WindowRef::Focused).ok().flatten(),
    };
    let settle = spec.settle;
    match window {
        Some(w) if settle.0.as_nanos() > 0 => {
            let wait = Wait::Settled {
                window: Some(WindowRef::id(w)),
                quiet: settle,
                timeout: Dur(settle.0 + spec.timeout.0),
            };
            state.start_wait(
                &wait,
                token,
                Box::new(move |state, result| prepare_when_clear(state, run, result.is_ok(), 0)),
            );
        }
        _ => {
            state.schedule_in(settle.0, move |state| {
                if !token.is_cancelled() {
                    prepare_when_clear(state, run, true, 0);
                }
            });
        }
    }
}

/// Prepares a repeat now, or after the next display deadline when its baseline readbacks could
/// make that deadline late.
fn prepare_when_clear(state: &mut State, run: Shared, settled: bool, deferrals: u32) {
    let (pixels, worst) = {
        let r = run.borrow();
        let pixels = matches!(
            r.spec.detector,
            Detector::PixelsChanged { .. } | Detector::PixelsMatch { .. }
        );
        (pixels, r.readback_max)
    };
    if pixels
        && deferrals < guard::MAX_DEFERRALS
        && let Some(at) = guard::readback_slot(state, guard::guard_for(worst))
    {
        state.schedule_at(at, move |state| {
            prepare_when_clear(state, run, settled, deferrals + 1);
        });
        return;
    }
    prepare_repeat(state, run, settled);
}

/// Stores the finished repeat and starts the next one.
pub(super) fn next_repeat(state: &mut State, run: Shared) {
    {
        let mut r = run.borrow_mut();
        if let Some(rep) = r.repeat.take() {
            r.report.results.push(rep.result);
        }
        r.iteration += 1;
    }
    begin_repeat(state, run);
}

/// The stage times the watch keeps per commit.
#[derive(Clone, Debug, Default)]
pub(super) struct CommitTimes {
    pub surface: Option<SurfaceId>,
    pub committed: Option<Timestamp>,
    pub ready: Option<Timestamp>,
    pub latched: Option<(Timestamp, u64)>,
    /// The presentation time and refresh sequence.
    pub presented: Option<(Timestamp, u64)>,
    /// The commit was discarded.
    pub discarded: bool,
}

/// Per-commit stage times by commit id.
pub(super) type CommitLog = HashMap<CommitId, CommitTimes>;
