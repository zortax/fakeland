//! Input groups: the effects of one logical input event, such as one pointer frame or one key
//! event, with the time the client received it and the compositor's input id.

use fakeland_marks::InputKind;
use fakeland_types::{InputEventId, Timestamp};
use serde::Serialize;

use super::{AxisFrame, InputEffect};

/// The kind of a logical input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    /// A pointer frame with motion, enter or leave and no button or axis.
    PointerMotion,
    /// A pointer frame with a button event.
    PointerButton,
    /// A pointer frame with axis events.
    PointerAxis,
    /// A key press or release.
    Key,
    /// A modifier state change without a key event.
    Modifiers,
    /// Keyboard focus entered the surface.
    KeyboardEnter,
    /// Keyboard focus left the surface.
    KeyboardLeave,
    /// A touch frame.
    Touch,
    /// A text-input-v3 done event with committed text.
    Text,
}

impl GroupKind {
    /// Returns the marks protocol kind that the compositor announces for this group, or `None`
    /// for groups that no injected event produces on its own.
    pub fn mark_kind(self) -> Option<InputKind> {
        Some(match self {
            Self::PointerMotion => InputKind::PointerMotion,
            Self::PointerButton => InputKind::PointerButton,
            Self::PointerAxis => InputKind::PointerAxis,
            Self::Key => InputKind::Key,
            Self::Touch => InputKind::Touch,
            Self::Text => InputKind::Text,
            Self::Modifiers | Self::KeyboardEnter | Self::KeyboardLeave => return None,
        })
    }

    /// Returns the kind of a pointer frame from the events it holds.
    pub fn of_pointer_frame(has_button: bool, axis: &AxisFrame) -> Self {
        if !axis.is_empty() {
            Self::PointerAxis
        } else if has_button {
            Self::PointerButton
        } else {
            Self::PointerMotion
        }
    }
}

/// The effects of one logical input event.
#[derive(Clone, Debug, PartialEq)]
pub struct InputGroup {
    /// The kind of the event.
    pub kind: GroupKind,
    /// The decoded effects in protocol order.
    pub effects: Vec<InputEffect>,
    /// The wl_pointer, wl_keyboard or wl_touch time in milliseconds, when the event has one.
    pub wl_time: Option<u32>,
    /// When the client dispatched the event.
    pub received: Timestamp,
    /// When the event loop woke up for the read that delivered the event.
    pub wake: Timestamp,
    /// The compositor's input id, when the compositor announced one.
    pub marks_id: Option<InputEventId>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pointer_frame_kind_prefers_axis_then_button() {
        let axis = AxisFrame {
            dy: 1.0,
            ..AxisFrame::default()
        };
        assert_eq!(
            GroupKind::of_pointer_frame(true, &axis),
            GroupKind::PointerAxis
        );
        assert_eq!(
            GroupKind::of_pointer_frame(true, &AxisFrame::default()),
            GroupKind::PointerButton
        );
        assert_eq!(
            GroupKind::of_pointer_frame(false, &AxisFrame::default()),
            GroupKind::PointerMotion
        );
    }

    #[test]
    fn only_injected_kinds_have_marks() {
        assert_eq!(GroupKind::Key.mark_kind(), Some(InputKind::Key));
        assert_eq!(GroupKind::Modifiers.mark_kind(), None);
        assert_eq!(GroupKind::KeyboardEnter.mark_kind(), None);
    }
}
