//! Touchpad gestures through `zwp_pointer_gestures_v1`: swipe, pinch and hold on the surface
//! under the pointer. Gesture events have no frame and go out at once.

use fakeland_protocol::server::fakeland_marks_manager_v1::InputKind as MarkKind;
use fakeland_trace::{InputKind, InputSource};
use smithay::input::pointer::{
    GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent, GesturePinchEndEvent,
    GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
};
use smithay::utils::{Point, SERIAL_COUNTER};

use super::{InjectError, PendingSent, SentInput};
use crate::server::State;

/// One gesture event.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GestureInput {
    /// A swipe begins.
    SwipeBegin(u32),
    /// A swipe moves by a delta.
    SwipeUpdate(f64, f64),
    /// A swipe ends.
    SwipeEnd(bool),
    /// A pinch begins.
    PinchBegin(u32),
    /// A pinch moves, scales to an absolute scale and rotates by a delta.
    PinchUpdate(f64, f64, f64, f64),
    /// A pinch ends.
    PinchEnd(bool),
    /// A hold begins.
    HoldBegin(u32),
    /// A hold ends.
    HoldEnd(bool),
}

impl GestureInput {
    fn kind(self) -> InputKind {
        match self {
            Self::SwipeBegin(_) => InputKind::SwipeBegin,
            Self::SwipeUpdate(..) => InputKind::SwipeUpdate,
            Self::SwipeEnd(_) => InputKind::SwipeEnd,
            Self::PinchBegin(_) => InputKind::PinchBegin,
            Self::PinchUpdate(..) => InputKind::PinchUpdate,
            Self::PinchEnd(_) => InputKind::PinchEnd,
            Self::HoldBegin(_) => InputKind::HoldBegin,
            Self::HoldEnd(_) => InputKind::HoldEnd,
        }
    }
}

impl State {
    /// Sends one gesture event to the pointer focus.
    pub(super) fn inject_gesture(
        &mut self,
        g: GestureInput,
        source: InputSource,
    ) -> Result<Vec<SentInput>, InjectError> {
        self.flush_deferred_motion();
        let mut sent = self.close_input_groups();
        let pointer = self.seat.get_pointer().ok_or(InjectError::NoPointer)?;
        let focus = pointer.current_focus();
        let (id, surface) = self.mark_input(focus.as_ref(), MarkKind::Gesture);
        let time = self.now().wayland_millis();
        let serial = SERIAL_COUNTER.next_serial();
        match g {
            GestureInput::SwipeBegin(fingers) => pointer.gesture_swipe_begin(
                self,
                &GestureSwipeBeginEvent {
                    serial,
                    time,
                    fingers,
                },
            ),
            GestureInput::SwipeUpdate(dx, dy) => pointer.gesture_swipe_update(
                self,
                &GestureSwipeUpdateEvent {
                    time,
                    delta: Point::from((dx, dy)),
                },
            ),
            GestureInput::SwipeEnd(cancelled) => pointer.gesture_swipe_end(
                self,
                &GestureSwipeEndEvent {
                    serial,
                    time,
                    cancelled,
                },
            ),
            GestureInput::PinchBegin(fingers) => pointer.gesture_pinch_begin(
                self,
                &GesturePinchBeginEvent {
                    serial,
                    time,
                    fingers,
                },
            ),
            GestureInput::PinchUpdate(dx, dy, scale, rotation) => pointer.gesture_pinch_update(
                self,
                &GesturePinchUpdateEvent {
                    time,
                    delta: Point::from((dx, dy)),
                    scale,
                    rotation,
                },
            ),
            GestureInput::PinchEnd(cancelled) => pointer.gesture_pinch_end(
                self,
                &GesturePinchEndEvent {
                    serial,
                    time,
                    cancelled,
                },
            ),
            GestureInput::HoldBegin(fingers) => pointer.gesture_hold_begin(
                self,
                &GestureHoldBeginEvent {
                    serial,
                    time,
                    fingers,
                },
            ),
            GestureInput::HoldEnd(cancelled) => pointer.gesture_hold_end(
                self,
                &GestureHoldEndEvent {
                    serial,
                    time,
                    cancelled,
                },
            ),
        }
        let position = self.windows.cursor.position;
        sent.extend(self.record_sent(vec![PendingSent {
            id,
            kind: g.kind(),
            surface,
            pos: Some((position.x, position.y)),
            detail: format!("{g:?}"),
            source,
        }]));
        Ok(sent)
    }
}
