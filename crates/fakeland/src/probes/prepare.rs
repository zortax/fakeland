//! Preparation of one probe repeat: target resolution, baseline readbacks, the watch and the
//! input at the chosen vblank phase.

use std::rc::Rc;

use fakeland_scenario::{Detector, WindowRef};
use fakeland_trace::Event;
use fakeland_types::{Logical, OutputId, Rect};

use super::ProbeResult;
use super::detect::{Goal, Watch};
use crate::actions::ActionCtx;
use crate::server::State;
use crate::util::CancelToken;
use crate::waits::Condition;

use super::run::{Repeat, Shared, Target, begin_repeat, detector_name, detector_roi, next_repeat};
use super::watch::{arm_watch, finish_repeat};

/// Resolves the target, takes baselines, arms the watch and schedules the input.
pub(super) fn prepare_repeat(state: &mut State, run: Shared, settled: bool) {
    let (spec, iteration, ctx) = {
        let r = run.borrow();
        (r.spec.clone(), r.iteration, r.ctx.clone())
    };
    if ctx.token.is_cancelled() {
        return;
    }
    let probe = state.ids.probe();
    state.emit(Event::ProbeStarted {
        probe,
        kind: detector_name(&spec.detector).to_string(),
    });
    let mut result = ProbeResult {
        probe: Some(probe),
        iteration,
        settled,
        ..ProbeResult::default()
    };
    let resolved = resolve_target(state, &spec.detector);
    let (target, roi, output) = match resolved {
        Some(r) => (r.target, r.roi, r.output),
        None => {
            result.timed_out = true;
            state.emit(Event::ProbeTimedOut { probe });
            run.borrow_mut().report.results.push(result);
            run.borrow_mut().iteration += 1;
            begin_repeat(state, run);
            return;
        }
    };
    let goal = match &spec.detector {
        Detector::AnyDamage { .. } => Goal::Damage,
        Detector::PixelsChanged { threshold, .. } => Goal::Changed(*threshold),
        Detector::PixelsMatch { condition, .. } => match Condition::new(condition) {
            Ok(c) => Goal::Match(c),
            Err(e) => {
                tracing::warn!("probe condition: {e}");
                Goal::Damage
            }
        },
        Detector::AppMark { name } => Goal::Mark(name.clone()),
    };
    let mut watch = Watch::new(target, roi, goal);
    let (reads, cost) = watch.take_baselines(state);
    if reads > 0 {
        let mut r = run.borrow_mut();
        r.readback_max = r.readback_max.max(cost / i64::from(reads));
    }
    result.readbacks += reads;
    result.readback_cost += cost;
    let period = output
        .and_then(|o| state.outputs.get(o))
        .and_then(|o| o.scheduler.period());
    let requested = period.and_then(|p| spec.phase.phase_for(iteration, p));
    result.requested_phase = requested;
    let now = state.now();
    let at = match (requested, output.and_then(|o| state.outputs.get(o))) {
        (Some(phase), Some(out)) => out
            .scheduler
            .next_vblank_after(now)
            .map_or(now, |g| g.time + phase),
        _ => now,
    };
    let token = CancelToken::new();
    run.borrow_mut().repeat = Some(Repeat {
        result,
        watch,
        token: token.clone(),
        hook: None,
        observer: None,
        timer: None,
        output,
        program_done: false,
        finished: false,
    });
    arm_watch(state, &run, &token);
    let deadline = at + spec.timeout.0;
    let timer_run = run.clone();
    let timer_token = token.clone();
    let timer = state.schedule_at(deadline, move |state| {
        if !timer_token.is_cancelled() {
            finish_repeat(state, &timer_run);
        }
    });
    if let Some(rep) = run.borrow_mut().repeat.as_mut() {
        rep.timer = Some(timer);
    }
    let input = if iteration % 2 == 1 {
        spec.alternate.clone().unwrap_or(spec.input.clone())
    } else {
        spec.input.clone()
    };
    let sent_run = run.clone();
    let program_ctx = ActionCtx::new(ctx.source)
        .with_token(ctx.token.clone())
        .with_sent_hook(Rc::new(
            move |state: &mut State, sent: &[crate::input::SentInput]| {
                let mut r = sent_run.borrow_mut();
                let Some(rep) = r.repeat.as_mut() else {
                    return;
                };
                if rep.finished {
                    return;
                }
                for s in sent {
                    rep.result.inputs.push(s.id);
                    rep.result.input_sent.get_or_insert(s.time);
                    rep.watch.started = true;
                    state.emit(Event::ProbeInput { probe, input: s.id });
                }
            },
        ));
    let done_run = run.clone();
    state.schedule_at(at, move |state| {
        if program_ctx.token.is_cancelled() {
            return;
        }
        state.perform(
            &input,
            program_ctx,
            Box::new(move |state, outcome| {
                if let Err(e) = outcome {
                    tracing::warn!("probe input failed: {e}");
                    finish_repeat(state, &done_run);
                }
                let next = {
                    let mut r = done_run.borrow_mut();
                    match r.repeat.as_mut() {
                        Some(rep) => {
                            rep.program_done = true;
                            rep.finished
                        }
                        None => false,
                    }
                };
                if next {
                    next_repeat(state, done_run);
                }
            }),
        );
    });
}

/// The target, the global region and the output of a detector.
struct Resolved {
    target: Target,
    roi: Option<Rect<Logical>>,
    output: Option<OutputId>,
}

/// Resolves the detector region against the current layout.
fn resolve_target(state: &State, detector: &Detector) -> Option<Resolved> {
    match detector_roi(detector) {
        Some(roi) => {
            let t = state
                .resolve_roi(roi)
                .map_err(|e| tracing::warn!("probe region: {e}"))
                .ok()?;
            let target = match t.window {
                Some(w) => Target::Window(w),
                None => Target::Output(t.output),
            };
            Some(Resolved {
                target,
                roi: Some(t.rect),
                output: Some(t.output),
            })
        }
        None => {
            let output = state
                .resolve_window(&WindowRef::Focused)
                .ok()
                .flatten()
                .and_then(|w| state.windows.get(w))
                .map(|w| w.output)
                .or_else(|| state.outputs.primary());
            Some(Resolved {
                target: Target::Any,
                roi: None,
                output,
            })
        }
    }
}
