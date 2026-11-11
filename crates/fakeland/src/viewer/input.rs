//! Host input of the viewer windows as session input: each event becomes injected primitives on
//! the output of its window, recorded with the viewer as source.

use fakeland_scenario::{Button, Key, Position, Primitive, ScrollSource};
use fakeland_trace::InputSource;
use fakeland_types::OutputId;
use smithay::input::keyboard::xkb;

use super::{AxisKind, ViewerInput};
use crate::input::KeymapLookup;
use crate::server::State;

/// Returns the evdev code of the session key with the keysym of a host key, or the host code
/// when the session keymap has no key with that keysym on its base level.
pub fn session_code(keymap: &KeymapLookup, code: u32, keysym: Option<u32>) -> u32 {
    keysym
        .and_then(|k| keymap.base_keycode(xkb::Keysym::new(k)))
        .map_or(code, |kc| kc.saturating_sub(8))
}

/// Returns the primitives of one viewer input event. Keys carry session codes already.
pub fn primitives(output: OutputId, input: ViewerInput) -> Vec<Primitive> {
    match input {
        ViewerInput::Motion { x, y } => vec![Primitive::PointerMotion {
            pos: Position::on_output(output, x, y),
        }],
        ViewerInput::Button { code, pressed } => vec![Primitive::PointerButton {
            button: Button::from_code(code),
            pressed,
        }],
        ViewerInput::Axis {
            dx,
            dy,
            value120,
            kind,
            stop,
        } => vec![Primitive::PointerAxis {
            dx,
            dy,
            source: match kind {
                AxisKind::Wheel => ScrollSource::wheel(value120.is_some()),
                AxisKind::Finger => ScrollSource::finger(true),
                AxisKind::Continuous => ScrollSource::Continuous,
            },
            value120,
            stop,
        }],
        ViewerInput::Key { code, pressed, .. } => vec![Primitive::Key {
            key: Key::Code(code),
            pressed,
        }],
    }
}

impl State {
    /// Injects one viewer input event.
    pub fn inject_viewer_input(&mut self, output: OutputId, input: ViewerInput) {
        let input = match input {
            ViewerInput::Key {
                code,
                keysym,
                pressed,
            } => ViewerInput::Key {
                code: session_code(&self.input.keymap, code, keysym),
                keysym,
                pressed,
            },
            other => other,
        };
        if let Err(e) = self.inject_batch(&primitives(output, input), InputSource::Viewer) {
            tracing::debug!("viewer input dropped: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_follow_the_keysym_into_the_session_layout() {
        let us = KeymapLookup::from_names("", "", "us", "", None).unwrap();
        // A host key that types z maps to the z key of a US session.
        assert_eq!(session_code(&us, 21, Some(xkb::keysyms::KEY_z)), 44);
        assert_eq!(session_code(&us, 1, None), 1);
        assert_eq!(session_code(&us, 7, Some(xkb::keysyms::KEY_adiaeresis)), 7);
    }

    #[test]
    fn maps_every_input_kind() {
        let o = OutputId(2);
        assert!(matches!(
            primitives(o, ViewerInput::Motion { x: 10.0, y: 20.0 })[..],
            [Primitive::PointerMotion {
                pos: Position::Output {
                    output: Some(OutputId(2)),
                    x: 10.0,
                    y: 20.0
                }
            }]
        ));
        assert!(matches!(
            primitives(
                o,
                ViewerInput::Button {
                    code: 0x110,
                    pressed: true
                }
            )[..],
            [Primitive::PointerButton {
                button: Button::Left,
                pressed: true
            }]
        ));
        let axis = primitives(
            o,
            ViewerInput::Axis {
                dx: 0.0,
                dy: 15.0,
                value120: Some((0, 120)),
                kind: AxisKind::Wheel,
                stop: false,
            },
        );
        assert!(matches!(
            axis[..],
            [Primitive::PointerAxis {
                source: ScrollSource::Wheel { hires: true, .. },
                value120: Some((0, 120)),
                ..
            }]
        ));
        assert!(matches!(
            &primitives(
                o,
                ViewerInput::Key {
                    code: 30,
                    keysym: None,
                    pressed: false
                }
            )[..],
            [Primitive::Key {
                key: Key::Code(30),
                pressed: false
            }]
        ));
    }
}
