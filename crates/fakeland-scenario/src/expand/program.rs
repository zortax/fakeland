//! The entry point of expansion: one action in, one input program out.

use super::{ExpandError, InputProgram, Primitive};
use super::{gesture, keyboard, pointer, resize, scroll, touch, typing};
use crate::{Action, KeyAction, PointerAction, TouchAction, WindowAction};

/// Expands an input action into a time-ordered program. Returns `None` for actions that inject
/// no input, such as waits, marks and output changes.
pub fn expand(action: &Action) -> Result<Option<InputProgram>, ExpandError> {
    let program = match action {
        Action::Pointer(a) => match a {
            PointerAction::MoveTo { pos } => {
                let mut p = InputProgram::new();
                pointer::motion(&mut p, fakeland_types::Nanos::ZERO, pos.clone());
                p
            }
            PointerAction::Button { button, pressed } => {
                let mut p = InputProgram::new();
                pointer::button(&mut p, fakeland_types::Nanos::ZERO, *button, *pressed);
                p
            }
            PointerAction::Click {
                at,
                button,
                count,
                interval,
                hold,
            } => pointer::click(at.as_ref(), *button, *count, *interval, *hold)?,
            PointerAction::Axis {
                dx,
                dy,
                source,
                value120,
            } => pointer::axis(*dx, *dy, *source, *value120)?,
        },
        Action::Key(a) => match a {
            KeyAction::Press { key } => keyboard::single(key, true),
            KeyAction::Release { key } => keyboard::single(key, false),
            KeyAction::Tap { key, hold } => keyboard::tap(key, *hold)?,
            KeyAction::Combo { combo, hold, gap } => keyboard::combo(combo, *hold, *gap)?,
        },
        Action::Type { text, cadence, via } => typing::type_text(text, cadence, *via)?,
        Action::Touch(a) => match a {
            TouchAction::Down { id, pos } => touch::single(Primitive::TouchDown {
                id: *id,
                pos: pos.clone(),
            }),
            TouchAction::Motion { id, pos } => touch::single(Primitive::TouchMotion {
                id: *id,
                pos: pos.clone(),
            }),
            TouchAction::Up { id } => touch::single(Primitive::TouchUp { id: *id }),
            TouchAction::Tap { id, pos, hold } => touch::tap(*id, pos, *hold)?,
            TouchAction::Glide {
                id,
                path,
                duration,
                easing,
                rate_hz,
            } => touch::glide(*id, path, *duration, *easing, *rate_hz)?,
        },
        Action::Glide(g) => pointer::glide(g)?,
        Action::ScrollGlide(g) => scroll::scroll_glide(g)?,
        Action::Drag(d) => pointer::drag(d)?,
        Action::Fling(f) => scroll::fling(f)?,
        Action::Gesture(g) => gesture::gesture(g)?,
        Action::Window(WindowAction::Resize { window, size }) => resize::resize(window, *size)?,
        Action::Window(WindowAction::ResizeStorm {
            window,
            from,
            to,
            duration,
            rate_hz,
            easing,
        }) => resize::storm(window, *from, *to, *duration, *rate_hz, *easing)?,
        _ => return Ok(None),
    };
    Ok(Some(program))
}
