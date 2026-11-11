//! A ping-pong latency loop: move the pointer between two points, and send each move when the
//! frame that answered the previous one presents.

use fakeland::reactor::ReactorCx;
use fakeland::scenario::{Position, Primitive};
use fakeland::{Event, Reactor, Record};
use fakeland_trace::AppMarkKind;
use fakeland_types::{CommitId, InputEventId, SurfaceId, Timestamp};
use serde::{Deserialize, Serialize};

/// One round of the loop.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PingPongSample {
    /// The round, from 0.
    pub round: u32,
    /// The input event.
    pub input: InputEventId,
    /// The send time of the move.
    pub sent: Timestamp,
    /// The commit that answered it.
    pub commit: CommitId,
    /// The commit time.
    pub committed: Timestamp,
    /// The modeled presentation of the answer.
    pub presented: Timestamp,
}

/// Moves the pointer to `a`, waits for the presentation of the commit that answers the move,
/// then moves to `b`, and so on for `rounds` rounds. With `marks`, the answer is the commit that
/// marks the move handled; without, it is the next commit with a buffer on the window surface.
/// Each round emits a [`PingPongSample`]; the reactor finishes after the last round.
#[derive(Clone, Debug)]
pub struct PingPong {
    /// The first point.
    pub a: Position,
    /// The second point.
    pub b: Position,
    /// The rounds.
    pub rounds: u32,
    /// The answer is the commit that marks the move handled.
    pub marks: bool,
    /// The surface whose commits answer, without marks; `None` takes any surface.
    pub surface: Option<SurfaceId>,
    round: u32,
    pending: Option<(InputEventId, Timestamp)>,
    answer: Option<(Option<CommitId>, SurfaceId, Timestamp)>,
}

impl PingPong {
    /// Builds the loop.
    pub fn new(a: Position, b: Position, rounds: u32) -> Self {
        Self {
            a,
            b,
            rounds,
            marks: true,
            surface: None,
            round: 0,
            pending: None,
            answer: None,
        }
    }

    /// Takes the next commit with a buffer on a surface as the answer, for apps without marks.
    pub fn without_marks(mut self, surface: Option<SurfaceId>) -> Self {
        self.marks = false;
        self.surface = surface;
        self
    }

    fn send(&mut self, cx: &mut ReactorCx<'_, PingPongSample>) {
        let pos = if self.round.is_multiple_of(2) {
            &self.a
        } else {
            &self.b
        };
        let batch = [
            Primitive::PointerMotion { pos: pos.clone() },
            Primitive::PointerFrame,
        ];
        match cx.inject(&batch) {
            Ok(sent) => {
                self.pending = sent.first().map(|s| (s.id, s.time));
                self.answer = None;
            }
            Err(_) => cx.finish(),
        }
    }
}

impl Reactor for PingPong {
    type Value = PingPongSample;

    fn on_start(&mut self, cx: &mut ReactorCx<'_, PingPongSample>) {
        if self.rounds == 0 {
            cx.finish();
        } else {
            self.send(cx);
        }
    }

    fn on_record(&mut self, record: &Record, cx: &mut ReactorCx<'_, PingPongSample>) {
        let Some((input, sent)) = self.pending else {
            return;
        };
        match &record.event {
            Event::AppMark {
                commit: Some(c),
                surface,
                mark: AppMarkKind::InputHandled { input: i },
            } if self.marks && *i == input && self.answer.is_none() => {
                self.answer = surface.map(|s| (Some(*c), s, record.t));
            }
            Event::Commit {
                surface,
                commit,
                buffer: Some(_),
                ..
            } if !self.marks
                && self.answer.is_none()
                && record.t >= sent
                && self.surface.is_none_or(|s| s == *surface) =>
            {
                self.answer = Some((Some(*commit), *surface, record.t));
            }
            Event::Discarded { surface, commit } => {
                // A discarded answer shows on screen with the next presented frame.
                if let Some(a) = self
                    .answer
                    .as_mut()
                    .filter(|a| a.0 == Some(*commit) && a.1 == *surface)
                {
                    a.0 = None;
                }
            }
            Event::Presented {
                surface,
                commit,
                time,
                ..
            } => {
                let Some((c, s, committed)) = self.answer else {
                    return;
                };
                if s != *surface || c.is_some_and(|c| c != *commit) {
                    return;
                }
                cx.emit(PingPongSample {
                    round: self.round,
                    input,
                    sent,
                    commit: *commit,
                    committed,
                    presented: *time,
                });
                self.pending = None;
                self.round += 1;
                if self.round >= self.rounds {
                    cx.finish();
                } else {
                    self.send(cx);
                }
            }
            _ => {}
        }
    }
}
