//! Input programs on the timer queue. An input action expands into timed primitives; events at
//! one offset form one batch, and each batch runs at its exact deadline. Positions resolve when
//! their batch runs. A path with mixed coordinate frames resolves to output coordinates first.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use fakeland_scenario::{Action, ExpandError, InputProgram, MotionError, Primitive, expand};
use fakeland_types::{Nanos, Timestamp};

use super::{ActionCtx, ActionDone, ActionError, ActionOutcome};
use crate::input::{SentInput, SentSink};
use crate::server::State;

/// A program in flight.
struct Job {
    start: Timestamp,
    batches: Vec<(Nanos, Vec<Primitive>)>,
    next: usize,
    sent: Vec<SentInput>,
    ctx: ActionCtx,
    done: Option<ActionDone>,
    /// Takes every record of the job, also those of motions that go out after their batch.
    sink: Option<SentSink>,
}

/// Returns the sink of a job: it keeps the records and passes them to the sent hook.
fn job_sink(job: Weak<RefCell<Job>>) -> SentSink {
    Rc::new(move |state: &mut State, sent: &[SentInput]| {
        let Some(job) = job.upgrade() else {
            return;
        };
        let hook = {
            let mut j = job.borrow_mut();
            j.sent.extend_from_slice(sent);
            j.ctx.on_sent.clone()
        };
        if let Some(hook) = hook {
            hook(state, sent);
        }
    })
}

/// Groups program events by offset, in order.
pub fn batches(program: InputProgram) -> Vec<(Nanos, Vec<Primitive>)> {
    let mut out: Vec<(Nanos, Vec<Primitive>)> = Vec::new();
    for e in program.events {
        match out.last_mut() {
            Some((at, list)) if *at == e.offset => list.push(e.primitive),
            _ => out.push((e.offset, vec![e.primitive])),
        }
    }
    out
}

impl State {
    /// Expands an input action and runs its program.
    pub(super) fn run_program(&mut self, action: &Action, ctx: ActionCtx, done: ActionDone) {
        let program = match self.expand_action(action) {
            Ok(p) => p,
            Err(e) => return done(self, Err(e)),
        };
        let job = Rc::new(RefCell::new(Job {
            start: self.now(),
            batches: batches(program),
            next: 0,
            sent: Vec::new(),
            ctx,
            done: Some(done),
            sink: None,
        }));
        let sink = job_sink(Rc::downgrade(&job));
        job.borrow_mut().sink = Some(sink);
        run_next(self, job);
    }

    fn expand_action(&self, action: &Action) -> Result<InputProgram, ActionError> {
        match expand(action) {
            Ok(Some(p)) => Ok(p),
            Ok(None) => Ok(InputProgram::new()),
            Err(ExpandError::Motion(MotionError::MixedFrames)) => {
                let mut resolved = action.clone();
                let mut failure = None;
                resolved.map_positions(&mut |p| match self.position_on_output(p) {
                    Ok(q) => q,
                    Err(e) => {
                        failure.get_or_insert(e);
                        p.clone()
                    }
                });
                if let Some(e) = failure {
                    return Err(e.into());
                }
                Ok(expand(&resolved)?.unwrap_or_default())
            }
            Err(e) => Err(e.into()),
        }
    }
}

/// Runs every batch that is due and schedules the next one.
fn run_next(state: &mut State, job: Rc<RefCell<Job>>) {
    loop {
        let (due, cancelled) = {
            let j = job.borrow();
            let due = j.batches.get(j.next).map(|(at, _)| j.start + *at);
            (due, j.ctx.token.is_cancelled())
        };
        if cancelled {
            return;
        }
        let Some(at) = due else {
            let waiting = job
                .borrow()
                .sink
                .as_ref()
                .and_then(|s| state.deferred_motion_for(s));
            if let Some(at) = waiting {
                let job = job.clone();
                state.schedule_at(at, move |state| run_next(state, job));
                return;
            }
            let (done, sent) = {
                let mut j = job.borrow_mut();
                (j.done.take(), std::mem::take(&mut j.sent))
            };
            if let Some(d) = done {
                d(state, Ok(ActionOutcome::Input { sent }));
            }
            return;
        };
        if at > state.now() {
            let job = job.clone();
            state.schedule_at(at, move |state| run_next(state, job));
            return;
        }
        let (primitives, ctx, sink) = {
            let mut j = job.borrow_mut();
            let i = j.next;
            j.next += 1;
            (
                std::mem::take(&mut j.batches[i].1),
                j.ctx.clone(),
                j.sink.clone(),
            )
        };
        match state.inject_batch_to(&primitives, ctx.source, sink.as_ref()) {
            Ok(sent) => {
                if let Some(sink) = &sink
                    && !sent.is_empty()
                {
                    sink(state, &sent);
                }
            }
            Err(e) => {
                let done = job.borrow_mut().done.take();
                if let Some(d) = done {
                    d(state, Err(e.into()));
                }
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use fakeland_scenario::{Button, Position};

    use super::*;

    #[test]
    fn batches_group_equal_offsets() {
        let mut p = InputProgram::new();
        p.push(
            Nanos::ZERO,
            Primitive::PointerMotion {
                pos: Position::output(1.0, 2.0),
            },
        );
        p.push(Nanos::ZERO, Primitive::PointerFrame);
        p.push(
            Nanos::from_millis(5),
            Primitive::PointerButton {
                button: Button::Left,
                pressed: true,
            },
        );
        p.push(Nanos::from_millis(5), Primitive::PointerFrame);
        p.push(Nanos::from_millis(9), Primitive::PointerFrame);
        let b = batches(p);
        assert_eq!(b.len(), 3);
        assert_eq!(b[0].0, Nanos::ZERO);
        assert_eq!(b[0].1.len(), 2);
        assert_eq!(b[1].0, Nanos::from_millis(5));
        assert_eq!(b[2].1, vec![Primitive::PointerFrame]);
    }
}
