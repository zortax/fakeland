//! Touch injection: points go down on the surface under them and keep that surface until they
//! lift. Events of one touch frame wait for `wl_touch.frame`.

use fakeland_protocol::server::fakeland_marks_manager_v1::InputKind as MarkKind;
use fakeland_scenario::Position;
use fakeland_trace::{InputKind, InputSource};
use smithay::input::touch::{DownEvent, MotionEvent, UpEvent};
use smithay::utils::SERIAL_COUNTER;

use super::{InjectError, PendingSent, TouchPoint};
use crate::server::State;

impl State {
    /// Puts a touch point down.
    pub(super) fn inject_touch_down(
        &mut self,
        slot: u32,
        pos: &Position,
        source: InputSource,
    ) -> Result<(), InjectError> {
        let touch = self.seat.get_touch().ok_or(InjectError::NoTouch)?;
        let (point, _) = self.resolve_position(pos)?;
        let focus = self.surface_under(point);
        if let Some(window) = self.window_under(point)
            && self.windows.focus != Some(window)
        {
            self.focus_window(Some(window));
        }
        let (id, surface) = self.mark_input(focus.as_ref().map(|(s, _)| s), MarkKind::Touch);
        touch.down(
            self,
            focus.clone(),
            &DownEvent {
                slot: Some(slot).into(),
                location: point,
                serial: SERIAL_COUNTER.next_serial(),
                time: self.now().wayland_millis(),
            },
        );
        self.input.touches.insert(slot, TouchPoint { focus });
        self.input.touch_group.push(PendingSent {
            id,
            kind: InputKind::TouchDown,
            surface,
            pos: Some((point.x, point.y)),
            detail: format!("slot {slot}"),
            source,
        });
        Ok(())
    }

    /// Moves a touch point.
    pub(super) fn inject_touch_motion(
        &mut self,
        slot: u32,
        pos: &Position,
        source: InputSource,
    ) -> Result<(), InjectError> {
        let touch = self.seat.get_touch().ok_or(InjectError::NoTouch)?;
        let focus = self
            .input
            .touches
            .get(&slot)
            .ok_or(InjectError::TouchNotDown(slot))?
            .focus
            .clone();
        let (point, _) = self.resolve_position(pos)?;
        let (id, surface) = self.mark_input(focus.as_ref().map(|(s, _)| s), MarkKind::Touch);
        touch.motion(
            self,
            focus,
            &MotionEvent {
                slot: Some(slot).into(),
                location: point,
                time: self.now().wayland_millis(),
            },
        );
        self.input.touch_group.push(PendingSent {
            id,
            kind: InputKind::TouchMotion,
            surface,
            pos: Some((point.x, point.y)),
            detail: format!("slot {slot}"),
            source,
        });
        Ok(())
    }

    /// Lifts a touch point.
    pub(super) fn inject_touch_up(
        &mut self,
        slot: u32,
        source: InputSource,
    ) -> Result<(), InjectError> {
        let touch = self.seat.get_touch().ok_or(InjectError::NoTouch)?;
        let point = self
            .input
            .touches
            .remove(&slot)
            .ok_or(InjectError::TouchNotDown(slot))?;
        let (id, surface) = self.mark_input(point.focus.as_ref().map(|(s, _)| s), MarkKind::Touch);
        touch.up(
            self,
            &UpEvent {
                slot: Some(slot).into(),
                serial: SERIAL_COUNTER.next_serial(),
                time: self.now().wayland_millis(),
            },
        );
        self.input.touch_group.push(PendingSent {
            id,
            kind: InputKind::TouchUp,
            surface,
            pos: None,
            detail: format!("slot {slot}"),
            source,
        });
        Ok(())
    }

    /// Lifts every touch point and releases every button, for the end of a run.
    pub fn release_held_pointer(&mut self, source: InputSource) {
        let slots: Vec<u32> = self.input.touches.keys().copied().collect();
        for slot in slots {
            let _ = self.inject_touch_up(slot, source);
        }
        let buttons = self.input.held_buttons.clone();
        for code in buttons {
            let _ = self.inject_button(fakeland_scenario::Button::from_code(code), false, source);
        }
        self.close_input_groups();
    }
}
