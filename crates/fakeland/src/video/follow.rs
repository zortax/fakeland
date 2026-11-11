//! The frames of a running recording. At each vblank of its output, the slots before the
//! vblank go to the writer with the picture from before it. A presentation of the target marks
//! the recording dirty, and the target composites and reads back on the next loop turn.

use std::sync::Arc;

use fakeland_trace::{Event, InputKind, Record};
use fakeland_types::Timestamp;

use super::RecordingId;
use super::registry::Source;
use super::writer::Push;
use crate::server::State;
use crate::waits::Observe;

impl State {
    pub(super) fn recording_record(&mut self, id: RecordingId, record: &Record) -> Observe {
        let Some(rec) = self.recordings.active.get(&id) else {
            return Observe::Remove;
        };
        let (source, output, cursor) = (rec.plan.source, rec.output, rec.plan.spec.include_cursor);
        match &record.event {
            Event::Vblank {
                output: o, ideal, ..
            } if *o == output => {
                self.advance_recording(id, *ideal, false);
            }
            Event::Presented {
                surface,
                output: o,
                time,
                ..
            } => {
                let shown = match source {
                    Source::Output(out) => *o == out,
                    Source::Window(w) => self.window_of_surface_id(*surface) == Some(w),
                };
                if shown {
                    self.advance_recording(id, *time, false);
                    self.dirty_recording(id);
                }
            }
            Event::WindowMapped { .. }
            | Event::WindowUnmapped { .. }
            | Event::OutputChanged(_)
            | Event::OutputPower { .. } => self.dirty_recording(id),
            Event::InputSent {
                kind: InputKind::PointerMotion | InputKind::PointerEnter,
                ..
            } if cursor => self.dirty_recording(id),
            _ => {}
        }
        Observe::Keep
    }

    /// Hands the slots before `t`, or through `t` at the end, to the writer.
    pub(super) fn advance_recording(&mut self, id: RecordingId, t: Timestamp, end: bool) {
        let Some(rec) = self.recordings.active.get_mut(&id) else {
            return;
        };
        let n = if end {
            rec.pacer.take_through(t)
        } else {
            rec.pacer.take_before(t)
        };
        if n == 0 {
            return;
        }
        let new = u64::from(rec.fresh);
        rec.rendered += new;
        rec.repeated += n - new;
        rec.fresh = false;
        if rec.queue.push(rec.current.clone(), n) == Push::Closed {
            tracing::debug!("recording {} lost its writer", id.0);
        }
    }

    fn dirty_recording(&mut self, id: RecordingId) {
        let Some(rec) = self.recordings.active.get_mut(&id) else {
            return;
        };
        rec.dirty = true;
        if rec.scheduled {
            return;
        }
        rec.scheduled = true;
        let now = self.now();
        self.schedule_at(now, move |state| state.composite_recording(id));
    }

    fn composite_recording(&mut self, id: RecordingId) {
        let Some(rec) = self.recordings.active.get_mut(&id) else {
            return;
        };
        rec.scheduled = false;
        if !rec.dirty {
            return;
        }
        rec.dirty = false;
        let (source, cursor, factor) = (
            rec.plan.source,
            rec.plan.spec.include_cursor,
            rec.plan.factor,
        );
        let start = self.now();
        let shot = self.capture_scaled(&source.target(), cursor, factor);
        let cost = self.now().since(start);
        let output = self.source_output(source);
        let Some(rec) = self.recordings.active.get_mut(&id) else {
            return;
        };
        rec.cost += cost;
        if let Some(o) = output {
            rec.output = o;
        }
        match shot {
            Ok(shot) => {
                rec.current = Arc::new(shot.image);
                rec.fresh = true;
            }
            Err(e) => tracing::debug!("recording {} keeps its last frame: {e}", id.0),
        }
    }
}
