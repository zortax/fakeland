//! The watch of one probe repeat: the commit hook that keeps candidate buffers and the buffers
//! that refresh baselines before the input, the observer that runs the detector, the target test
//! and the end of the repeat.

use fakeland_trace::Event;
use fakeland_types::SurfaceId;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::wayland::compositor::{BufferAssignment, SurfaceAttributes, with_states};

use super::ProbeStages;
use crate::frames::CommitPhase;
use crate::server::State;
use crate::util::CancelToken;
use crate::waits::Observe;

use super::run::{Shared, Target, next_repeat};

/// Installs the commit hook that keeps candidate buffers and the observer that runs the
/// detector.
pub(super) fn arm_watch(state: &mut State, run: &Shared, token: &CancelToken) {
    let hook_run = run.clone();
    let hook_token = token.clone();
    let hook = state.add_commit_hook(move |state, event| {
        if event.phase != CommitPhase::Committed || hook_token.is_cancelled() {
            return;
        }
        let wants = {
            let r = hook_run.borrow();
            r.repeat.as_ref().is_some_and(|rep| {
                rep.watch.wants_buffers() && (rep.watch.started || rep.watch.needs_baselines())
            })
        };
        if !wants || !in_target(state, &hook_run, event.id) {
            return;
        }
        let buffer: Option<WlBuffer> = with_states(event.surface, |s| {
            match &s.cached_state.get::<SurfaceAttributes>().pending().buffer {
                Some(BufferAssignment::NewBuffer(b)) => Some(b.clone()),
                _ => None,
            }
        });
        if let (Some(b), Some(&commit)) = (buffer, event.commits.first())
            && let Some(rep) = hook_run.borrow_mut().repeat.as_mut()
        {
            rep.watch.stash.insert(commit, b);
        }
    });
    let obs_run = run.clone();
    let obs_token = token.clone();
    let observer = state.observe(move |state, record| {
        if obs_token.is_cancelled() {
            return Observe::Remove;
        }
        let step = super::detect::on_record(state, &obs_run, record);
        if step == super::detect::Step::Finished {
            finish_repeat(state, &obs_run);
            return Observe::Remove;
        }
        Observe::Keep
    });
    if let Some(rep) = run.borrow_mut().repeat.as_mut() {
        rep.hook = Some(hook);
        rep.observer = Some(observer);
    }
}

/// Returns true when a surface belongs to the probe target.
pub(super) fn in_target(state: &State, run: &Shared, surface: SurfaceId) -> bool {
    let target = match run.borrow().repeat.as_ref() {
        Some(rep) => rep.watch.target,
        None => return false,
    };
    match target {
        Target::Any => true,
        Target::Window(w) => state.window_of_surface_id(surface) == Some(w),
        Target::Output(o) => state
            .window_of_surface_id(surface)
            .and_then(|w| state.windows.get(w))
            .is_some_and(|w| w.output == o),
    }
}

/// Ends the current repeat: computes its stages and removes its watch.
pub(super) fn finish_repeat(state: &mut State, run: &Shared) {
    let (hook, observer, timer, program_done, probe, timed_out) = {
        let mut r = run.borrow_mut();
        let Some(rep) = r.repeat.as_mut() else {
            return;
        };
        if rep.finished {
            return;
        }
        rep.finished = true;
        rep.token.cancel();
        let res = &mut rep.result;
        res.timed_out = res.detected_commit.is_none();
        res.stages = ProbeStages::compute(
            res.input_sent,
            res.committed,
            res.ready,
            res.latched,
            res.presented,
        );
        (
            rep.hook.take(),
            rep.observer.take(),
            rep.timer.take(),
            rep.program_done,
            res.probe,
            res.timed_out,
        )
    };
    let phase = {
        let r = run.borrow();
        r.repeat.as_ref().and_then(|rep| {
            let out = state.outputs.get(rep.output?)?;
            out.scheduler.phase_of(rep.result.input_sent?)
        })
    };
    if let Some(rep) = run.borrow_mut().repeat.as_mut() {
        rep.result.phase_in_frame = phase;
    }
    if let Some(h) = hook {
        state.remove_commit_hook(h);
    }
    if let Some(o) = observer {
        state.unobserve(o);
    }
    if let Some(t) = timer {
        state.cancel_timer(t);
    }
    if timed_out && let Some(p) = probe {
        state.emit(Event::ProbeTimedOut { probe: p });
    }
    if program_done {
        next_repeat(state, run.clone());
    }
}
