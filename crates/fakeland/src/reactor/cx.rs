//! The context of a reactor callback: input, actions, timers, captures, the vblank grid, window
//! queries, values for the handle and the end of the reactor.

use std::cell::Cell;
use std::rc::Rc;

use fakeland_scenario::{Action, CaptureTarget, Primitive, WindowRef};
use fakeland_timing::GridPoint;
use fakeland_trace::{Event, InputSource};
use fakeland_types::{Logical, Nanos, OutputId, Rect, Timestamp};
use futures_channel::mpsc::UnboundedSender;

use super::ReactorId;
use crate::actions::ActionCtx;
use crate::capture::{CaptureError, CaptureOptions, Screenshot};
use crate::framecap::{CaptureSpec, FrameCaptureId};
use crate::input::{InjectError, SentInput};
use crate::server::State;
use crate::shell::WindowInfo;

/// What a reactor callback can do.
pub struct ReactorCx<'a, V> {
    pub(super) state: &'a mut State,
    pub(super) id: ReactorId,
    pub(super) tx: &'a UnboundedSender<V>,
    pub(super) finished: &'a mut bool,
}

impl<V> std::fmt::Debug for ReactorCx<'_, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReactorCx").field("id", &self.id).finish()
    }
}

impl<V: Send + 'static> ReactorCx<'_, V> {
    /// Returns the id of the reactor.
    pub fn id(&self) -> ReactorId {
        self.id
    }

    /// Returns the current time.
    pub fn now(&self) -> Timestamp {
        self.state.now()
    }

    /// Injects primitives now and closes their groups. Returns the events that reached clients,
    /// with send times after the flush.
    pub fn inject(&mut self, primitives: &[Primitive]) -> Result<Vec<SentInput>, InjectError> {
        self.state.inject_batch(primitives, InputSource::Reactor)
    }

    /// Injects primitives at a time.
    pub fn inject_at(&mut self, at: Timestamp, primitives: Vec<Primitive>) {
        self.state.schedule_at(at, move |state| {
            if let Err(e) = state.inject_batch(&primitives, InputSource::Reactor) {
                tracing::warn!("reactor input failed: {e}");
            }
        });
    }

    /// Runs a scenario action now: input programs, output and window changes, marks, waits,
    /// probes and the clipboard. A failure goes to the log.
    pub fn perform(&mut self, action: Action) {
        let ctx = ActionCtx::new(InputSource::Reactor);
        self.state.perform(
            &action,
            ctx,
            Box::new(|_, r| {
                if let Err(e) = r {
                    tracing::warn!("reactor action failed: {e}");
                }
            }),
        );
    }

    /// Runs a scenario action at a time.
    pub fn perform_at(&mut self, at: Timestamp, action: Action) {
        self.state.schedule_at(at, move |state| {
            let ctx = ActionCtx::new(InputSource::Reactor);
            state.perform(
                &action,
                ctx,
                Box::new(|_, r| {
                    if let Err(e) = r {
                        tracing::warn!("reactor action failed: {e}");
                    }
                }),
            );
        });
    }

    /// Calls [`super::Reactor::on_timer`] with `token` at a time.
    pub fn wake_at(&mut self, at: Timestamp, token: u64) {
        let id = self.id;
        self.state
            .schedule_at(at, move |state| state.reactor_timer(id, token));
    }

    /// Captures a target now, synchronously. The cost lands in the trace.
    pub fn capture(&mut self, target: &CaptureTarget) -> Result<Screenshot, CaptureError> {
        self.state.capture(target, CaptureOptions::default())
    }

    /// Asks for a capture of a window region at its next applied commit (or next presentation
    /// with `presented`). The frame arrives in [`super::Reactor::on_capture`].
    pub fn capture_next_commit(
        &mut self,
        window: WindowRef,
        roi: Option<Rect<Logical>>,
        presented: bool,
    ) -> Result<(), CaptureError> {
        let mut spec = if presented {
            CaptureSpec::every_presented(window)
        } else {
            CaptureSpec::every_commit(window)
        }
        .with_max_frames(1);
        if let Some(r) = roi {
            spec = spec.with_roi(r);
        }
        let id = self.id;
        let slot: Rc<Cell<Option<FrameCaptureId>>> = Rc::new(Cell::new(None));
        let own = slot.clone();
        let fid = self.state.begin_frame_capture(
            spec,
            Some(Box::new(move |state, frame| {
                if let Some(f) = own.take() {
                    state.end_frame_capture(f);
                }
                state.reactor_capture(id, frame);
            })),
        )?;
        slot.set(Some(fid));
        Ok(())
    }

    /// Returns the next vblank of an output after now. `None` selects the primary output.
    pub fn next_vblank(&self, output: Option<OutputId>) -> Option<GridPoint> {
        let out = self
            .state
            .outputs
            .get(self.state.outputs.resolve(output)?)?;
        out.scheduler.next_vblank_after(self.state.now())
    }

    /// Returns the last vblank of an output.
    pub fn last_vblank(&self, output: Option<OutputId>) -> Option<GridPoint> {
        let out = self
            .state
            .outputs
            .get(self.state.outputs.resolve(output)?)?;
        out.scheduler.last_vblank()
    }

    /// Returns the refresh period of an output.
    pub fn period(&self, output: Option<OutputId>) -> Option<Nanos> {
        let out = self
            .state
            .outputs
            .get(self.state.outputs.resolve(output)?)?;
        out.scheduler.period()
    }

    /// Returns the phase of a time within the refresh period of an output.
    pub fn phase(&self, output: Option<OutputId>, t: Timestamp) -> Option<Nanos> {
        let out = self
            .state
            .outputs
            .get(self.state.outputs.resolve(output)?)?;
        out.scheduler.phase_of(t)
    }

    /// Returns the first mapped window that matches.
    pub fn window(&self, r: &WindowRef) -> Option<WindowInfo> {
        let id = self.state.resolve_window(r).ok()??;
        self.state
            .windows
            .get(id)
            .map(|rec| self.state.window_info(rec))
    }

    /// Returns every window.
    pub fn windows(&self) -> Vec<WindowInfo> {
        self.state.window_infos()
    }

    /// Records a user mark.
    pub fn mark(&mut self, label: impl Into<String>) {
        self.state.emit(Event::UserMark {
            label: label.into(),
        });
    }

    /// Sends a value to the handle.
    pub fn emit(&mut self, value: V) {
        let _ = self.tx.unbounded_send(value);
    }

    /// Removes the reactor after this callback. The handle drains the values sent so far and
    /// then ends.
    pub fn finish(&mut self) {
        *self.finished = true;
    }
}
