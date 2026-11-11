//! Kinds and sources of injected input events.

use fakeland_types::RunId;
use serde::{Deserialize, Serialize};

/// The kind of an input event sent to a client.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum InputKind {
    /// `wl_pointer.enter`.
    PointerEnter,
    /// `wl_pointer.leave`.
    PointerLeave,
    /// `wl_pointer.motion`.
    PointerMotion,
    /// `zwp_relative_pointer_v1.relative_motion`.
    RelativeMotion,
    /// `wl_pointer.button`.
    PointerButton,
    /// `wl_pointer.axis` and its value120, source and stop events.
    PointerAxis,
    /// `wl_pointer.frame`.
    PointerFrame,
    /// `wl_keyboard.enter`.
    KeyboardEnter,
    /// `wl_keyboard.leave`.
    KeyboardLeave,
    /// `wl_keyboard.key`.
    Key,
    /// `wl_keyboard.modifiers`.
    Modifiers,
    /// Text through `zwp_text_input_v3` commit or a temporary keymap.
    Text,
    /// `wl_touch.down`.
    TouchDown,
    /// `wl_touch.motion`.
    TouchMotion,
    /// `wl_touch.up`.
    TouchUp,
    /// `wl_touch.frame`.
    TouchFrame,
    /// `wl_touch.cancel`.
    TouchCancel,
    /// `zwp_pointer_gesture_swipe_v1.begin`.
    SwipeBegin,
    /// `zwp_pointer_gesture_swipe_v1.update`.
    SwipeUpdate,
    /// `zwp_pointer_gesture_swipe_v1.end`.
    SwipeEnd,
    /// `zwp_pointer_gesture_pinch_v1.begin`.
    PinchBegin,
    /// `zwp_pointer_gesture_pinch_v1.update`.
    PinchUpdate,
    /// `zwp_pointer_gesture_pinch_v1.end`.
    PinchEnd,
    /// `zwp_pointer_gesture_hold_v1.begin`.
    HoldBegin,
    /// `zwp_pointer_gesture_hold_v1.end`.
    HoldEnd,
}

impl InputKind {
    /// Returns the snake_case name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PointerEnter => "pointer_enter",
            Self::PointerLeave => "pointer_leave",
            Self::PointerMotion => "pointer_motion",
            Self::RelativeMotion => "relative_motion",
            Self::PointerButton => "pointer_button",
            Self::PointerAxis => "pointer_axis",
            Self::PointerFrame => "pointer_frame",
            Self::KeyboardEnter => "keyboard_enter",
            Self::KeyboardLeave => "keyboard_leave",
            Self::Key => "key",
            Self::Modifiers => "modifiers",
            Self::Text => "text",
            Self::TouchDown => "touch_down",
            Self::TouchMotion => "touch_motion",
            Self::TouchUp => "touch_up",
            Self::TouchFrame => "touch_frame",
            Self::TouchCancel => "touch_cancel",
            Self::SwipeBegin => "swipe_begin",
            Self::SwipeUpdate => "swipe_update",
            Self::SwipeEnd => "swipe_end",
            Self::PinchBegin => "pinch_begin",
            Self::PinchUpdate => "pinch_update",
            Self::PinchEnd => "pinch_end",
            Self::HoldBegin => "hold_begin",
            Self::HoldEnd => "hold_end",
        }
    }

    /// Returns true for events that carry user intent. Enter, leave and frame events return false.
    pub const fn is_actionable(self) -> bool {
        !matches!(
            self,
            Self::PointerEnter
                | Self::PointerLeave
                | Self::PointerFrame
                | Self::KeyboardEnter
                | Self::KeyboardLeave
                | Self::Modifiers
                | Self::TouchFrame
        )
    }
}

/// The origin of an injected input event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputSource {
    /// A direct library or control socket call.
    Api,
    /// A step of a scenario run.
    Scenario {
        /// The scenario run.
        run: RunId,
    },
    /// A trigger inside the compositor.
    Trigger,
    /// A reactor installed on the compositor thread.
    Reactor,
    /// The nested viewer window.
    Viewer,
}

impl InputSource {
    /// Returns the snake_case name of the source kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Scenario { .. } => "scenario",
            Self::Trigger => "trigger",
            Self::Reactor => "reactor",
            Self::Viewer => "viewer",
        }
    }
}
