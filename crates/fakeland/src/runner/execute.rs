//! The scenario executor on the compositor thread. Steps start at their anchors on the timer
//! queue or when an anchor wait holds; each step runs one action, and the next step follows when
//! it finishes. Triggers stay armed and frame captures run for the whole scenario. A failure, the
//! scenario deadline or a cancel ends the run.

use fakeland_scenario::{Action, Scenario};
use fakeland_trace::{Event, InputSource};
use fakeland_types::RunId;
use futures_channel::oneshot;

use super::anchor::AnchorPlan;
use super::{CaptureResult, CurrentStep, Cursor, Run, RunError, RunOptions, RunResult, StepTiming};
use crate::actions::{ActionCtx, ActionError, ActionOutcome};
use crate::server::State;
use crate::util::CancelToken;

/// Returns the JSON type name of an action.
pub fn action_name(action: &Action) -> &'static str {
    match action {
        Action::Pointer(_) => "pointer",
        Action::Key(_) => "key",
        Action::Type { .. } => "type",
        Action::Touch(_) => "touch",
        Action::Glide(_) => "glide",
        Action::ScrollGlide(_) => "scroll_glide",
        Action::Drag(_) => "drag",
        Action::Fling(_) => "fling",
        Action::Gesture(_) => "gesture",
        Action::Output(_) => "output",
        Action::Window(_) => "window",
        Action::Mark { .. } => "mark",
        Action::Capture { .. } => "capture",
        Action::Probe(_) => "probe",
        Action::Wait(_) => "wait",
        Action::Sleep { .. } => "sleep",
        Action::Repeat { .. } => "repeat",
        Action::Clipboard(_) => "clipboard",
    }
}

impl State {
    /// Starts a scenario run. The result goes to `reply` when the run ends.
    pub fn start_run(
        &mut self,
        scenario: Scenario,
        options: RunOptions,
        reply: oneshot::Sender<RunResult>,
    ) -> RunId {
        let id = self.ids.run();
        let now = self.now();
        let capture = self.bus.begin_capture();
        self.emit(Event::ScenarioStarted {
            run: id,
            name: scenario.name.clone(),
        });
        let run = Run {
            name: scenario.name.clone(),
            started: now,
            capture,
            cursor: Cursor::new(scenario.steps.clone()),
            token: CancelToken::new(),
            generation: 0,
            current: None,
            prev_finished: now,
            index: 0,
            steps: Vec::new(),
            probes: Vec::new(),
            captures: Vec::new(),
            triggers: None,
            trigger_observer: None,
            trigger_log: Vec::new(),
            frames: Vec::new(),
            timeout: None,
            reply: Some(reply),
            in_begin: false,
            sync_done: None,
        };
        self.runs.active.insert(id, run);
        if let Err(e) = scenario.validate() {
            self.finish_run(id, Err(RunError::Invalid(e)));
            return id;
        }
        for spec in options.frames {
            match self.begin_frame_capture(spec, None) {
                Ok(f) => {
                    if let Some(run) = self.runs.active.get_mut(&id) {
                        run.frames.push(f);
                    }
                }
                Err(e) => tracing::warn!("frame capture not started: {e}"),
            }
        }
        if let Some(timeout) = scenario.timeout {
            let t = self.schedule_at(now + timeout.0, move |state| {
                state.finish_run(id, Err(RunError::Timeout));
            });
            if let Some(run) = self.runs.active.get_mut(&id) {
                run.timeout = Some(t);
            }
        }
        self.arm_triggers(id, &scenario.triggers);
        self.advance_run(id);
        id
    }

    /// Cancels a run. Returns false when no such run is active.
    pub fn cancel_run(&mut self, id: RunId) -> bool {
        if !self.runs.active.contains_key(&id) {
            return false;
        }
        self.finish_run(id, Err(RunError::Cancelled));
        true
    }

    /// Ends every run, for shutdown.
    pub fn close_runs(&mut self) {
        let ids: Vec<RunId> = self.runs.active.keys().copied().collect();
        for id in ids {
            self.finish_run(id, Err(RunError::Closed));
        }
    }

    /// Starts steps until one has to wait.
    fn advance_run(&mut self, id: RunId) {
        loop {
            let now = self.now();
            let Some(run) = self.runs.active.get_mut(&id) else {
                return;
            };
            let Some(next) = run.cursor.next_step() else {
                self.finish_run(id, Ok(()));
                return;
            };
            run.generation += 1;
            let generation = run.generation;
            let index = run.index;
            run.index += 1;
            let (started, prev) = (run.started, run.prev_finished);
            let plan = self.plan_anchor(&next.step, started, prev);
            let scheduled = match &plan {
                AnchorPlan::At(t) => (*t).max(prev),
                AnchorPlan::Wait(_) => now,
            };
            let Some(run) = self.runs.active.get_mut(&id) else {
                return;
            };
            run.current = Some(CurrentStep {
                next,
                index,
                scheduled,
                started: None,
            });
            let token = run.token.clone();
            match plan {
                AnchorPlan::At(t) if t <= now => {
                    if !self.begin_step(id, generation) {
                        return;
                    }
                }
                AnchorPlan::At(t) => {
                    self.schedule_at(t, move |state| {
                        if state.begin_step(id, generation) {
                            state.advance_run(id);
                        }
                    });
                    return;
                }
                AnchorPlan::Wait(wait) => {
                    self.start_wait(
                        &wait,
                        token,
                        Box::new(move |state, result| match result {
                            Ok(out) => {
                                if let Some(c) = state
                                    .runs
                                    .active
                                    .get_mut(&id)
                                    .and_then(|r| r.current.as_mut())
                                {
                                    c.scheduled = out.time;
                                }
                                if state.begin_step(id, generation) {
                                    state.advance_run(id);
                                }
                            }
                            Err(source) => {
                                let path = state.current_path(id);
                                state.finish_run(id, Err(RunError::Anchor { path, source }));
                            }
                        }),
                    );
                    return;
                }
            }
        }
    }

    fn current_path(&self, id: RunId) -> String {
        self.runs
            .active
            .get(&id)
            .and_then(|r| r.current.as_ref())
            .map(|c| c.next.path.clone())
            .unwrap_or_default()
    }

    /// Starts the action of the current step. Returns true when the step finished at once and
    /// the run goes on.
    fn begin_step(&mut self, id: RunId, generation: u64) -> bool {
        let now = self.now();
        let Some(run) = self.runs.active.get_mut(&id) else {
            return false;
        };
        if run.generation != generation {
            return false;
        }
        let Some(current) = run.current.as_mut() else {
            return false;
        };
        current.started = Some(now);
        let (index, step) = (current.index, current.next.clone());
        let token = run.token.clone();
        self.emit(Event::ScenarioStep {
            run: id,
            index,
            label: step.step.label.clone().unwrap_or_else(|| step.path.clone()),
        });
        if let Action::Repeat { count, steps } = &step.step.action {
            if let Some(run) = self.runs.active.get_mut(&id) {
                run.cursor.enter_repeat(steps, *count, &step.path);
            }
            return self.step_finished(id, Ok(ActionOutcome::Done));
        }
        let ctx = ActionCtx::new(InputSource::Scenario { run: id }).with_token(token);
        if let Some(run) = self.runs.active.get_mut(&id) {
            run.in_begin = true;
            run.sync_done = None;
        }
        self.perform(
            &step.step.action,
            ctx,
            Box::new(move |state, result| state.step_done(id, generation, result)),
        );
        let sync = match self.runs.active.get_mut(&id) {
            Some(run) => {
                run.in_begin = false;
                run.sync_done.take()
            }
            None => return false,
        };
        match sync {
            Some(result) => self.step_finished(id, result),
            None => false,
        }
    }

    /// Takes the outcome of a step action.
    fn step_done(
        &mut self,
        id: RunId,
        generation: u64,
        result: Result<ActionOutcome, ActionError>,
    ) {
        let Some(run) = self.runs.active.get_mut(&id) else {
            return;
        };
        if run.generation != generation {
            return;
        }
        if run.in_begin {
            run.sync_done = Some(result);
            return;
        }
        if self.step_finished(id, result) {
            self.advance_run(id);
        }
    }

    /// Records a finished step. Returns true when the run goes on.
    fn step_finished(&mut self, id: RunId, result: Result<ActionOutcome, ActionError>) -> bool {
        let now = self.now();
        let Some(run) = self.runs.active.get_mut(&id) else {
            return false;
        };
        let Some(current) = run.current.take() else {
            return false;
        };
        let outcome = match result {
            Ok(o) => o,
            Err(source) => {
                let path = current.next.path.clone();
                self.finish_run(id, Err(RunError::Step { path, source }));
                return false;
            }
        };
        let mut inputs = 0;
        match outcome {
            ActionOutcome::Input { sent } => inputs = sent.len(),
            ActionOutcome::Captured { label, screenshot } => run.captures.push(CaptureResult {
                path: current.next.path.clone(),
                label,
                screenshot,
            }),
            ActionOutcome::Probe(report) => run.probes.push(report),
            ActionOutcome::Done | ActionOutcome::Waited(_) => {}
        }
        let started = current.started.unwrap_or(now);
        run.steps.push(StepTiming {
            index: current.index,
            path: current.next.path,
            iteration: current.next.iteration,
            label: current.next.step.label,
            action: action_name(&current.next.step.action).to_string(),
            scheduled: current.scheduled.min(started),
            started,
            finished: now,
            inputs,
        });
        run.prev_finished = now;
        true
    }

    /// Ends a run and sends its result.
    pub(crate) fn finish_run(&mut self, id: RunId, result: Result<(), RunError>) {
        let Some(mut run) = self.runs.active.remove(&id) else {
            return;
        };
        run.token.cancel();
        if let Some(t) = run.timeout.take() {
            self.cancel_timer(t);
        }
        if let Some(o) = run.trigger_observer.take() {
            self.unobserve(o);
        }
        let frames = run
            .frames
            .iter()
            .filter_map(|f| self.end_frame_capture(*f))
            .collect();
        if result.is_err() {
            self.release_held_input(InputSource::Scenario { run: id });
        }
        let (ok, error, kind) = match &result {
            Ok(()) => (true, None, None),
            Err(e) => (false, Some(e.to_string()), Some(e.kind())),
        };
        self.emit(Event::ScenarioFinished {
            run: id,
            ok,
            error: error.clone(),
        });
        let finished = self.now();
        let trace = self
            .bus
            .end_capture(run.capture)
            .unwrap_or_else(|_| self.bus.recorder().snapshot());
        let out = RunResult {
            run: id,
            name: run.name,
            ok,
            error,
            error_kind: kind,
            started: run.started,
            finished,
            steps: run.steps,
            probes: run.probes,
            triggers: run.trigger_log,
            captures: run.captures,
            frames,
            trace,
        };
        if let Some(reply) = run.reply.take() {
            let _ = reply.send(out);
        }
    }
}
