//! wl_keyboard events. The keymap compiles with xkbcommon; each key event becomes one input
//! group with its keysym and text. Modifier, enter and leave events are logged as groups without
//! effects. The client implements no key repeat.

use wayland_client::protocol::wl_keyboard::{self, WlKeyboard};
use wayland_client::{Connection, Dispatch, QueueHandle, WEnum};

use super::KeyState;
use crate::app::App;
use crate::input::{GroupKind, InputEffect};

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
            wl_keyboard::Event::Keymap { format, fd, size } => {
                if !matches!(format, WEnum::Value(wl_keyboard::KeymapFormat::XkbV1)) {
                    app.seat.keys = None;
                    return;
                }
                match KeyState::from_fd(fd, size) {
                    Ok(keys) => app.seat.keys = Some(keys),
                    Err(e) => {
                        app.seat.keys = None;
                        app.warn(format!("keymap: {e}"));
                    }
                }
            }
            wl_keyboard::Event::Enter { .. } => {
                app.input_group(GroupKind::KeyboardEnter, Vec::new(), None);
            }
            wl_keyboard::Event::Leave { .. } => {
                app.input_group(GroupKind::KeyboardLeave, Vec::new(), None);
            }
            wl_keyboard::Event::Key {
                time, key, state, ..
            } => {
                let pressed = matches!(
                    state,
                    WEnum::Value(wl_keyboard::KeyState::Pressed | wl_keyboard::KeyState::Repeated)
                );
                let (keysym, utf8) = app
                    .seat
                    .keys
                    .as_ref()
                    .map_or((0, String::new()), |k| k.key(key));
                let effect = InputEffect::Key {
                    code: key,
                    keysym,
                    utf8,
                    pressed,
                };
                app.input_group(GroupKind::Key, vec![effect], Some(time));
            }
            wl_keyboard::Event::Modifiers {
                mods_depressed,
                mods_latched,
                mods_locked,
                group,
                ..
            } => {
                if let Some(keys) = &mut app.seat.keys {
                    keys.update_mask(mods_depressed, mods_latched, mods_locked, group);
                }
                app.input_group(GroupKind::Modifiers, Vec::new(), None);
            }
            _ => {}
        }
    }
}
