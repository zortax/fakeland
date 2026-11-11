//! The seat: wl_seat capabilities create and release the pointer, keyboard and touch objects,
//! and the text-input-v3 object when the type scene runs.

use wayland_client::protocol::wl_keyboard::WlKeyboard;
use wayland_client::protocol::wl_pointer::WlPointer;
use wayland_client::protocol::wl_seat::{self, WlSeat};
use wayland_client::protocol::wl_touch::WlTouch;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::ZwpTextInputV3;

use super::KeyState;
use crate::app::App;
use crate::input::{InputEffect, PointerAccumulator};

/// The input objects and their decoding state.
#[derive(Debug, Default)]
pub struct SeatState {
    /// The pointer.
    pub pointer: Option<WlPointer>,
    /// The keyboard.
    pub keyboard: Option<WlKeyboard>,
    /// The touch device.
    pub touch: Option<WlTouch>,
    /// The text input.
    pub text_input: Option<ZwpTextInputV3>,
    /// The open pointer frame.
    pub pointer_frame: PointerAccumulator,
    /// The keymap state.
    pub keys: Option<KeyState>,
    /// The effects of the open touch frame.
    pub touch_effects: Vec<InputEffect>,
    /// The latest time of the open touch frame.
    pub touch_time: Option<u32>,
    /// The text-input commit string that waits for done.
    pub commit_string: Option<String>,
}

impl Dispatch<WlSeat, ()> for App {
    fn event(
        app: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities { capabilities } = event else {
            return;
        };
        let caps = match capabilities {
            WEnum::Value(c) => c,
            WEnum::Unknown(bits) => wl_seat::Capability::from_bits_truncate(bits),
        };
        let s = &mut app.seat;
        match (
            caps.contains(wl_seat::Capability::Pointer),
            s.pointer.take(),
        ) {
            (true, None) => s.pointer = Some(seat.get_pointer(qh, ())),
            (true, Some(p)) => s.pointer = Some(p),
            (false, Some(p)) if p.version() >= 3 => p.release(),
            (false, _) => {}
        }
        match (
            caps.contains(wl_seat::Capability::Keyboard),
            s.keyboard.take(),
        ) {
            (true, None) => s.keyboard = Some(seat.get_keyboard(qh, ())),
            (true, Some(k)) => s.keyboard = Some(k),
            (false, Some(k)) if k.version() >= 3 => k.release(),
            (false, _) => {}
        }
        match (caps.contains(wl_seat::Capability::Touch), s.touch.take()) {
            (true, None) => s.touch = Some(seat.get_touch(qh, ())),
            (true, Some(t)) => s.touch = Some(t),
            (false, Some(t)) if t.version() >= 3 => t.release(),
            (false, _) => {}
        }
        if s.text_input.is_none()
            && app.scenes.wants_text()
            && let Some(manager) = &app.globals.text_input
        {
            s.text_input = Some(manager.get_text_input(seat, qh, ()));
        }
    }
}
