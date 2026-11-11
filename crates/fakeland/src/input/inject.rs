//! The entry point of injection: one primitive or a batch of primitives at one instant. Pointer
//! and touch events gather into groups that close with their frame, so the send time of every
//! record follows the flush of its group. A batch closes every open group at its end.

use fakeland_scenario::{Primitive, WindowRef};
use fakeland_trace::InputSource;

use super::{AxisInput, GestureInput, InjectError, SentInput, SentSink};
use crate::server::State;
use crate::shell::WindowOp;

/// The event family of a primitive, which decides when open groups close.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Pointer,
    Touch,
    Other,
}

fn family(p: &Primitive) -> Family {
    match p {
        Primitive::PointerMotion { .. }
        | Primitive::PointerButton { .. }
        | Primitive::PointerAxis { .. }
        | Primitive::PointerFrame => Family::Pointer,
        Primitive::TouchDown { .. }
        | Primitive::TouchMotion { .. }
        | Primitive::TouchUp { .. }
        | Primitive::TouchFrame => Family::Touch,
        _ => Family::Other,
    }
}

impl State {
    /// Injects primitives in order, then closes every open group. Returns the records of every
    /// event that reached a client. A motion that waits for its report slot goes out later
    /// with only its trace record.
    pub fn inject_batch(
        &mut self,
        primitives: &[Primitive],
        source: InputSource,
    ) -> Result<Vec<SentInput>, InjectError> {
        self.inject_batch_to(primitives, source, None)
    }

    /// Injects primitives like [`State::inject_batch`]. A motion that waits for its report slot
    /// hands its record to `sink` when it goes out.
    pub fn inject_batch_to(
        &mut self,
        primitives: &[Primitive],
        source: InputSource,
        sink: Option<&SentSink>,
    ) -> Result<Vec<SentInput>, InjectError> {
        let mut sent = Vec::new();
        for p in primitives {
            if let Err(e) = self.inject_one(p, source, sink, &mut sent) {
                sent.extend(self.close_input_groups());
                return Err(e);
            }
        }
        sent.extend(self.close_input_groups());
        Ok(sent)
    }

    fn inject_one(
        &mut self,
        p: &Primitive,
        source: InputSource,
        sink: Option<&SentSink>,
        sent: &mut Vec<SentInput>,
    ) -> Result<(), InjectError> {
        let f = family(p);
        if (f != Family::Pointer && !self.input.pointer_group.is_empty())
            || (f != Family::Touch && !self.input.touch_group.is_empty())
        {
            sent.extend(self.close_input_groups());
        }
        match p {
            Primitive::PointerMotion { pos } => self.inject_motion(pos, source, sink)?,
            Primitive::PointerButton { button, pressed } => {
                self.inject_button(*button, *pressed, source)?
            }
            Primitive::PointerAxis {
                dx,
                dy,
                source: axis_source,
                value120,
                stop,
            } => self.inject_axis(
                AxisInput {
                    dx: *dx,
                    dy: *dy,
                    source: *axis_source,
                    value120: *value120,
                    stop: *stop,
                },
                source,
            )?,
            Primitive::PointerFrame => {
                if !self.input.pointer_group.is_empty() {
                    sent.extend(self.close_input_groups());
                }
            }
            Primitive::Key { key, pressed } => sent.extend(self.inject_key(key, *pressed, source)?),
            Primitive::Text { text } => sent.extend(self.inject_text(text, source)?),
            Primitive::TouchDown { id, pos } => self.inject_touch_down(*id, pos, source)?,
            Primitive::TouchMotion { id, pos } => self.inject_touch_motion(*id, pos, source)?,
            Primitive::TouchUp { id } => self.inject_touch_up(*id, source)?,
            Primitive::TouchFrame => {
                if !self.input.touch_group.is_empty() {
                    sent.extend(self.close_input_groups());
                }
            }
            Primitive::SwipeBegin { fingers } => {
                sent.extend(self.inject_gesture(GestureInput::SwipeBegin(*fingers), source)?)
            }
            Primitive::SwipeUpdate { dx, dy } => {
                sent.extend(self.inject_gesture(GestureInput::SwipeUpdate(*dx, *dy), source)?)
            }
            Primitive::SwipeEnd { cancelled } => {
                sent.extend(self.inject_gesture(GestureInput::SwipeEnd(*cancelled), source)?)
            }
            Primitive::PinchBegin { fingers } => {
                sent.extend(self.inject_gesture(GestureInput::PinchBegin(*fingers), source)?)
            }
            Primitive::PinchUpdate {
                dx,
                dy,
                scale,
                rotation,
            } => sent.extend(self.inject_gesture(
                GestureInput::PinchUpdate(*dx, *dy, *scale, *rotation),
                source,
            )?),
            Primitive::PinchEnd { cancelled } => {
                sent.extend(self.inject_gesture(GestureInput::PinchEnd(*cancelled), source)?)
            }
            Primitive::HoldBegin { fingers } => {
                sent.extend(self.inject_gesture(GestureInput::HoldBegin(*fingers), source)?)
            }
            Primitive::HoldEnd { cancelled } => {
                sent.extend(self.inject_gesture(GestureInput::HoldEnd(*cancelled), source)?)
            }
            Primitive::WindowResize { window, size } => self.resize_window(window, *size)?,
        }
        Ok(())
    }

    fn resize_window(
        &mut self,
        window: &WindowRef,
        size: fakeland_types::Size<fakeland_types::Logical>,
    ) -> Result<(), InjectError> {
        let id = self.resolve_window(window)?.ok_or(InjectError::NoWindow)?;
        self.window_op(id, WindowOp::Resize { size })?;
        Ok(())
    }

    /// Releases every held key, button and touch point.
    pub fn release_held_input(&mut self, source: InputSource) {
        self.release_held_keys(source);
        self.release_held_pointer(source);
    }
}
