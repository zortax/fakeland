//! Seat input: pointer enter, leave and buttons, keyboard focus and keys, each with the name of
//! the surface it reached.

use serde_json::json;
use wayland_client::protocol::wl_keyboard::{self, KeyState, WlKeyboard};
use wayland_client::protocol::wl_pointer::{self, ButtonState, WlPointer};
use wayland_client::protocol::wl_seat::{self, Capability, WlSeat};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};

use crate::app::App;
use crate::log;

impl Dispatch<WlSeat, ()> for App {
    fn event(
        _: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        {
            if caps.contains(Capability::Pointer) {
                seat.get_pointer(qh, ());
            }
            if caps.contains(Capability::Keyboard) {
                seat.get_keyboard(qh, ());
            }
        }
    }
}

impl Dispatch<WlPointer, ()> for App {
    fn event(
        app: &mut Self,
        _: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                surface,
                surface_x,
                surface_y,
                ..
            } => {
                let name = app.surface_name(&surface);
                app.pointer = (surface_x, surface_y);
                log::event(
                    "pointer_enter",
                    json!({ "surface": name, "x": surface_x, "y": surface_y }),
                );
                app.pointer_surface = Some(name);
            }
            wl_pointer::Event::Leave { surface, .. } => {
                log::event(
                    "pointer_leave",
                    json!({ "surface": app.surface_name(&surface) }),
                );
                app.pointer_surface = None;
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => app.pointer = (surface_x, surface_y),
            wl_pointer::Event::Button {
                button,
                state: WEnum::Value(state),
                ..
            } => {
                let pressed = state == ButtonState::Pressed;
                log::event(
                    "button",
                    json!({
                        "surface": app.pointer_surface,
                        "button": button,
                        "pressed": pressed,
                        "x": app.pointer.0,
                        "y": app.pointer.1,
                    }),
                );
                if pressed && app.args.unlock_on_input {
                    app.unlock();
                }
            }
            wl_pointer::Event::Axis { axis, value, .. } => log::event(
                "axis",
                json!({ "surface": app.pointer_surface, "axis": format!("{axis:?}"), "value": value }),
            ),
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for App {
    fn event(
        app: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Enter { surface, .. } => log::event(
                "keyboard_enter",
                json!({ "surface": app.surface_name(&surface) }),
            ),
            wl_keyboard::Event::Leave { surface, .. } => log::event(
                "keyboard_leave",
                json!({ "surface": app.surface_name(&surface) }),
            ),
            wl_keyboard::Event::Key {
                key,
                state: WEnum::Value(state),
                ..
            } => {
                let pressed = state == KeyState::Pressed;
                log::event("key", json!({ "key": key, "pressed": pressed }));
                if pressed && app.args.unlock_on_input {
                    app.unlock();
                }
            }
            _ => {}
        }
    }
}
