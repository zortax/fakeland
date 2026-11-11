//! Matching of compositor input ids to the input events that the client handles.
//!
//! The compositor announces each injected input event with an id right before the event, on the
//! same connection. The ids of one socket read arrive in the same order as the events. A handler
//! takes the oldest id when its family (pointer, keyboard or touch) matches; otherwise the event
//! carries no id. The client dispatches an event microseconds after it reads the event's id, so
//! an id that waits longer belongs to no handled event, and `sweep` drops it.

use std::collections::VecDeque;

use fakeland_marks::{InputKind, InputMark};
use fakeland_types::Timestamp;

/// A device family of input events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// wl_pointer events.
    Pointer,
    /// wl_keyboard and text-input events.
    Keyboard,
    /// wl_touch events.
    Touch,
}

impl Family {
    /// Returns the family of a marks kind, or `None` for kinds that the client never handles.
    pub fn of(kind: InputKind) -> Option<Self> {
        match kind {
            InputKind::PointerMotion | InputKind::PointerButton | InputKind::PointerAxis => {
                Some(Self::Pointer)
            }
            InputKind::Key | InputKind::Text => Some(Self::Keyboard),
            InputKind::Touch => Some(Self::Touch),
            _ => None,
        }
    }
}

/// The ids that wait for their input event.
#[derive(Debug, Default)]
pub struct MarkMatcher {
    queue: VecDeque<(Timestamp, InputMark)>,
}

/// The result of one match.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Matched {
    /// The id of the handled event.
    pub mark: Option<InputMark>,
    /// Ids of kinds that the client never handles, dropped from the front of the queue.
    pub skipped: Vec<InputMark>,
}

impl MarkMatcher {
    /// Appends ids in arrival order. `now` is the time the client read them.
    pub fn push(&mut self, now: Timestamp, marks: impl IntoIterator<Item = InputMark>) {
        self.queue.extend(marks.into_iter().map(|m| (now, m)));
    }

    /// Returns the number of waiting ids.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Returns true when no id waits.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Takes the id for a handled event of a marks kind.
    pub fn take(&mut self, kind: InputKind) -> Matched {
        let mut out = Matched::default();
        let Some(family) = Family::of(kind) else {
            return out;
        };
        while let Some((_, front)) = self.queue.front() {
            if front.kind.and_then(Family::of).is_some() {
                break;
            }
            out.skipped.extend(self.queue.pop_front().map(|(_, m)| m));
        }
        if let Some((_, front)) = self.queue.front()
            && front.kind.and_then(Family::of) == Some(family)
        {
            out.mark = self.queue.pop_front().map(|(_, m)| m);
        }
        out
    }

    /// Drops and returns the ids read before `cutoff`.
    pub fn sweep(&mut self, cutoff: Timestamp) -> Vec<InputMark> {
        let mut out = Vec::new();
        while self.queue.front().is_some_and(|(t, _)| *t < cutoff) {
            out.extend(self.queue.pop_front().map(|(_, m)| m));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::InputEventId;

    use super::*;

    fn mark(id: u64, kind: InputKind) -> InputMark {
        InputMark {
            id: InputEventId(id),
            kind: Some(kind),
        }
    }

    #[test]
    fn takes_in_order() {
        let mut m = MarkMatcher::default();
        m.push(
            Timestamp(0),
            [
                mark(1, InputKind::PointerMotion),
                mark(2, InputKind::PointerButton),
            ],
        );
        assert_eq!(
            m.take(InputKind::PointerMotion).mark,
            Some(mark(1, InputKind::PointerMotion))
        );
        // A button frame classified as motion still matches within the pointer family.
        assert_eq!(
            m.take(InputKind::PointerMotion).mark,
            Some(mark(2, InputKind::PointerButton))
        );
        assert!(m.is_empty());
    }

    #[test]
    fn other_family_front_gives_no_id() {
        let mut m = MarkMatcher::default();
        m.push(Timestamp(0), [mark(1, InputKind::Key)]);
        let r = m.take(InputKind::PointerMotion);
        assert_eq!(r.mark, None);
        assert_eq!(m.len(), 1);
        assert_eq!(m.take(InputKind::Key).mark, Some(mark(1, InputKind::Key)));
    }

    #[test]
    fn unhandled_kinds_are_skipped() {
        let mut m = MarkMatcher::default();
        m.push(
            Timestamp(0),
            [
                mark(1, InputKind::Gesture),
                InputMark {
                    id: InputEventId(2),
                    kind: None,
                },
                mark(3, InputKind::Touch),
            ],
        );
        let r = m.take(InputKind::Touch);
        assert_eq!(r.mark, Some(mark(3, InputKind::Touch)));
        assert_eq!(r.skipped.len(), 2);
    }

    #[test]
    fn text_and_key_share_a_family() {
        let mut m = MarkMatcher::default();
        m.push(Timestamp(0), [mark(7, InputKind::Text)]);
        assert_eq!(m.take(InputKind::Key).mark, Some(mark(7, InputKind::Text)));
    }

    #[test]
    fn sweep_drops_leftovers() {
        let mut m = MarkMatcher::default();
        m.push(
            Timestamp(0),
            [mark(1, InputKind::Key), mark(2, InputKind::Touch)],
        );
        m.push(Timestamp(10), [mark(3, InputKind::Key)]);
        assert_eq!(m.sweep(Timestamp(5)).len(), 2);
        assert_eq!(m.len(), 1);
        assert_eq!(m.sweep(Timestamp(11)).len(), 1);
        assert!(m.is_empty());
    }
}
