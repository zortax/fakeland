//! Injected events on their way to the trace. Events of one pointer or touch group wait for the
//! group frame; the flush to the client sockets happens first, so the send time of each
//! `InputSent` record follows the flush.

use std::rc::Rc;

use fakeland_trace::{Event, InputKind, InputSource};
use fakeland_types::{InputEventId, SurfaceId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::server::State;

/// One injected input event as the trace records it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SentInput {
    /// The input id. App marks name it in `input_handled`.
    pub id: InputEventId,
    /// The kind.
    pub kind: InputKind,
    /// The send time, after the flush to the client socket.
    pub time: Timestamp,
    /// The surface that received the event.
    pub surface: Option<SurfaceId>,
}

/// A callback that takes the records of injected events right after their flush.
pub type SentSink = Rc<dyn Fn(&mut State, &[SentInput])>;

/// An event that is sent and waits for its flush.
#[derive(Clone, Debug)]
pub struct PendingSent {
    /// The input id.
    pub id: InputEventId,
    /// The kind.
    pub kind: InputKind,
    /// The surface that received the event.
    pub surface: Option<SurfaceId>,
    /// The global logical position, for positional events.
    pub pos: Option<(f64, f64)>,
    /// A human readable description.
    pub detail: String,
    /// What injected the event.
    pub source: InputSource,
}

impl State {
    /// Flushes the client sockets, then records every pending event with the send time.
    pub fn record_sent(&mut self, pending: Vec<PendingSent>) -> Vec<SentInput> {
        if pending.is_empty() {
            return Vec::new();
        }
        self.flush_clients();
        let t = self.now();
        pending
            .into_iter()
            .map(|p| {
                let time = self.emit_at(
                    t,
                    Event::InputSent {
                        id: p.id,
                        kind: p.kind,
                        surface: p.surface,
                        x: p.pos.map(|(x, _)| x),
                        y: p.pos.map(|(_, y)| y),
                        detail: p.detail,
                        source: p.source,
                    },
                );
                SentInput {
                    id: p.id,
                    kind: p.kind,
                    time,
                    surface: p.surface,
                }
            })
            .collect()
    }

    /// Ends the open pointer and touch groups with their frames and records their events.
    pub fn close_input_groups(&mut self) -> Vec<SentInput> {
        let mut out = Vec::new();
        if !self.input.pointer_group.is_empty() {
            if let Some(pointer) = self.seat.get_pointer() {
                pointer.frame(self);
            }
            let group = std::mem::take(&mut self.input.pointer_group);
            out.extend(self.record_sent(group));
        }
        if !self.input.touch_group.is_empty() {
            if let Some(touch) = self.seat.get_touch() {
                touch.frame(self);
            }
            let group = std::mem::take(&mut self.input.touch_group);
            out.extend(self.record_sent(group));
        }
        out
    }
}
