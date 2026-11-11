//! Dispatch of one action. Input actions run as programs; output, window, mark and clipboard
//! actions finish at once; captures, sleeps, waits and probes finish later through the event
//! loop, the timer queue and observers.

use std::sync::Arc;

use fakeland_scenario::{Action, ClipboardAction};
use fakeland_trace::Event;
use fakeland_types::OutputId;

use super::{ActionCtx, ActionDone, ActionError, ActionOutcome};
use crate::capture::CaptureStart;
use crate::outputs::{OutputChange, OutputError};
use crate::server::State;
use crate::shell::WindowOp;

impl State {
    /// Runs one action. `done` runs once with the outcome, unless the token is cancelled first.
    pub fn perform(&mut self, action: &Action, ctx: ActionCtx, done: ActionDone) {
        if ctx.token.is_cancelled() {
            return;
        }
        if action.is_input() {
            return self.run_program(action, ctx, done);
        }
        let result = match action {
            Action::Output(a) => {
                let (output, change) = OutputChange::from_action(a);
                self.outputs
                    .resolve(output)
                    .ok_or(OutputError::NoOutput(output.unwrap_or(OutputId(0))))
                    .and_then(|id| self.change_output(id, change))
                    .map(|()| ActionOutcome::Done)
                    .map_err(ActionError::from)
            }
            Action::Window(a) => match WindowOp::from_action(a) {
                Some((window, op)) => self.window_action(&window, op),
                None => Err(ActionError::Unsupported("a resize storm")),
            },
            Action::Mark { label } => {
                self.emit(Event::UserMark {
                    label: label.clone(),
                });
                Ok(ActionOutcome::Done)
            }
            Action::Capture {
                target,
                label,
                max_dim,
            } => {
                let (label, max_dim) = (label.clone(), *max_dim);
                let token = ctx.token.clone();
                self.capture_then(
                    target.clone(),
                    false,
                    CaptureStart::Now,
                    move |state, result| {
                        if token.is_cancelled() {
                            return;
                        }
                        let result = result
                            .map(|shot| ActionOutcome::Captured {
                                label,
                                screenshot: shot.downscaled(max_dim),
                            })
                            .map_err(ActionError::from);
                        done(state, result);
                    },
                );
                return;
            }
            Action::Clipboard(ClipboardAction::Set { mime, text }) => {
                let bytes: Arc<[u8]> = Arc::from(text.as_bytes());
                self.offer_selection(mime, bytes);
                Ok(ActionOutcome::Done)
            }
            Action::Sleep { duration } => {
                let token = ctx.token.clone();
                self.schedule_in(duration.0, move |state| {
                    if !token.is_cancelled() {
                        done(state, Ok(ActionOutcome::Done));
                    }
                });
                return;
            }
            Action::Wait(wait) => {
                let token = ctx.token.clone();
                self.start_wait(
                    wait,
                    token,
                    Box::new(move |state, r| {
                        done(state, r.map(ActionOutcome::Waited).map_err(Into::into))
                    }),
                );
                return;
            }
            Action::Probe(spec) => {
                self.start_probe(
                    spec,
                    ctx,
                    Box::new(move |state, report| done(state, Ok(ActionOutcome::Probe(report)))),
                );
                return;
            }
            Action::Repeat { .. } => Err(ActionError::Unsupported("repeat")),
            _ => Err(ActionError::Unsupported("this action")),
        };
        done(self, result);
    }

    fn window_action(
        &mut self,
        window: &fakeland_scenario::WindowRef,
        op: WindowOp,
    ) -> Result<ActionOutcome, ActionError> {
        let id = self.resolve_window(window)?.ok_or(ActionError::NoWindow)?;
        self.window_op(id, op)?;
        Ok(ActionOutcome::Done)
    }
}
