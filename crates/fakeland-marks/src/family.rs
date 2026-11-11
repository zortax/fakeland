//! Matching of announced input ids to the input handlers of an app. An app handles input in
//! families: a pointer frame, a key event, a touch event or a text-input commit. The handler of a
//! family takes the announced ids of that family from the head of the queue.

use std::collections::VecDeque;

use fakeland_types::InputEventId;

use crate::input::{InputKind, InputMark};

/// A group of input kinds that one input handler of an app processes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Family {
    /// wl_pointer events up to wl_pointer.frame: motion, enter, button and axis.
    Pointer,
    /// wl_keyboard.key events.
    Key,
    /// wl_touch events.
    Touch,
    /// zwp_pointer_gestures_v1 events.
    Gesture,
    /// zwp_text_input_v3 commit_string and done.
    Text,
}

impl Family {
    /// Returns true when an input kind belongs to this family.
    pub fn contains(self, kind: InputKind) -> bool {
        matches!(
            (self, kind),
            (
                Self::Pointer,
                InputKind::PointerMotion | InputKind::PointerButton | InputKind::PointerAxis
            ) | (Self::Key, InputKind::Key)
                | (Self::Touch, InputKind::Touch)
                | (Self::Gesture, InputKind::Gesture)
                | (Self::Text, InputKind::Text)
        )
    }

    fn matches(self, mark: &InputMark) -> bool {
        mark.kind.is_some_and(|kind| self.contains(kind))
    }
}

/// Takes the ids that belong to a handled event of a family.
///
/// The compositor announces each id right before its input event, so the queue holds the ids in
/// the order of the events on the connection. The first entry of the family is the id of the
/// handled event. The entries before it belong to events that the app dispatched earlier and did
/// not hand to a handler, so the function drops them. The consecutive entries of the family that
/// follow belong to the same event group or to events of the same dispatch, and the function
/// takes them too. When the queue holds no entry of the family, the function changes nothing.
pub(crate) fn take(queue: &mut VecDeque<InputMark>, family: Family) -> Vec<InputEventId> {
    let Some(first) = queue.iter().position(|m| family.matches(m)) else {
        return Vec::new();
    };
    queue.drain(..first);
    let count = queue.iter().take_while(|m| family.matches(m)).count();
    queue.drain(..count).map(|m| m.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queue(entries: &[(u64, Option<InputKind>)]) -> VecDeque<InputMark> {
        entries
            .iter()
            .map(|&(id, kind)| InputMark {
                id: InputEventId(id),
                kind,
            })
            .collect()
    }

    fn ids(list: &[u64]) -> Vec<InputEventId> {
        list.iter().copied().map(InputEventId).collect()
    }

    #[test]
    fn a_pointer_frame_takes_its_motion_and_button() {
        let mut q = queue(&[
            (1, Some(InputKind::PointerMotion)),
            (2, Some(InputKind::PointerButton)),
            (3, Some(InputKind::Key)),
        ]);
        assert_eq!(take(&mut q, Family::Pointer), ids(&[1, 2]));
        assert_eq!(take(&mut q, Family::Key), ids(&[3]));
        assert!(q.is_empty());
    }

    #[test]
    fn a_later_family_stays_queued() {
        let mut q = queue(&[(4, Some(InputKind::Key)), (5, Some(InputKind::Key))]);
        assert!(take(&mut q, Family::Pointer).is_empty());
        assert_eq!(q.len(), 2);
        assert_eq!(take(&mut q, Family::Key), ids(&[4, 5]));
    }

    #[test]
    fn unhandled_entries_before_the_family_drop() {
        let mut q = queue(&[
            (6, Some(InputKind::Gesture)),
            (7, None),
            (8, Some(InputKind::Touch)),
            (9, Some(InputKind::Key)),
        ]);
        assert_eq!(take(&mut q, Family::Touch), ids(&[8]));
        assert_eq!(take(&mut q, Family::Key), ids(&[9]));
        assert!(q.is_empty());
    }

    #[test]
    fn the_take_stops_at_another_family() {
        let mut q = queue(&[
            (10, Some(InputKind::PointerAxis)),
            (11, Some(InputKind::Text)),
            (12, Some(InputKind::PointerMotion)),
        ]);
        assert_eq!(take(&mut q, Family::Pointer), ids(&[10]));
        assert_eq!(take(&mut q, Family::Text), ids(&[11]));
        assert_eq!(take(&mut q, Family::Pointer), ids(&[12]));
    }

    #[test]
    fn families_contain_their_kinds() {
        assert!(Family::Pointer.contains(InputKind::PointerAxis));
        assert!(!Family::Pointer.contains(InputKind::Gesture));
        assert!(Family::Gesture.contains(InputKind::Gesture));
        assert!(Family::Text.contains(InputKind::Text));
        assert!(!Family::Key.contains(InputKind::Text));
    }
}
