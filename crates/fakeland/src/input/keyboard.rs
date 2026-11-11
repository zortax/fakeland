//! Keyboard injection. A key name resolves to a keycode in the current keymap, with the modifier
//! keys of its shift level. A keysym the keymap lacks goes through text input when the focused
//! client enabled it, or through a temporary keymap with the keysym on a free keycode. The base
//! keymap returns when no temporary key is down.

use fakeland_protocol::server::fakeland_marks_manager_v1::InputKind as MarkKind;
use fakeland_scenario::Key;
use fakeland_trace::{InputKind, InputSource};
use smithay::backend::input::KeyState;
use smithay::input::keyboard::{FilterResult, Keycode, xkb};
use smithay::utils::SERIAL_COUNTER;

use super::keymap::{keysym_by_name, keysym_char, legacy_keysym};
use super::{Chord, HeldKey, InjectError, PendingSent, SentInput, TempKeymap};
use crate::config::UnmappedKeys;
use crate::server::State;

/// How a key press reaches the client.
enum KeyPlan {
    /// A key of the current keymap.
    Chord(Chord),
    /// A character the keymap lacks, through text input.
    Text(char),
    /// A keysym the keymap lacks, through the temporary keymap.
    Temporary(xkb::Keysym),
}

impl State {
    /// Presses or releases a key.
    pub(super) fn inject_key(
        &mut self,
        key: &Key,
        pressed: bool,
        source: InputSource,
    ) -> Result<Vec<SentInput>, InjectError> {
        if pressed {
            self.press_key(key, source)
        } else {
            self.release_key(key, source)
        }
    }

    fn press_key(&mut self, key: &Key, source: InputSource) -> Result<Vec<SentInput>, InjectError> {
        let mut sent = Vec::new();
        match self.plan_key(key)? {
            KeyPlan::Chord(chord) => {
                let down: Vec<u32> = self
                    .input
                    .held_keys
                    .iter()
                    .flat_map(|h| std::iter::once(h.keycode).chain(h.implied.iter().copied()))
                    .collect();
                let implied: Vec<u32> = chord
                    .modifiers
                    .iter()
                    .copied()
                    .filter(|m| !down.contains(m))
                    .collect();
                for m in &implied {
                    sent.push(self.send_key(*m, true, "level modifier", source)?);
                }
                sent.push(self.send_key(chord.keycode, true, &key.to_string(), source)?);
                self.input.held_keys.push(HeldKey {
                    key: key.clone(),
                    keycode: chord.keycode,
                    implied,
                    temporary: false,
                });
            }
            KeyPlan::Text(c) => {
                sent.extend(self.inject_text_input(&c.to_string(), source));
            }
            KeyPlan::Temporary(sym) => {
                let keycode = self.temporary_keycode(sym, key)?;
                sent.push(self.send_key(keycode, true, &key.to_string(), source)?);
                self.input.held_keys.push(HeldKey {
                    key: key.clone(),
                    keycode,
                    implied: Vec::new(),
                    temporary: true,
                });
            }
        }
        Ok(sent)
    }

    fn release_key(
        &mut self,
        key: &Key,
        source: InputSource,
    ) -> Result<Vec<SentInput>, InjectError> {
        let mut sent = Vec::new();
        let Some(i) = self.input.held_keys.iter().rposition(|h| h.key == *key) else {
            if let KeyPlan::Chord(chord) = self.plan_key(key)? {
                sent.push(self.send_key(chord.keycode, false, &key.to_string(), source)?);
            }
            return Ok(sent);
        };
        let held = self.input.held_keys.remove(i);
        sent.push(self.send_key(held.keycode, false, &key.to_string(), source)?);
        for m in held.implied.iter().rev() {
            sent.push(self.send_key(*m, false, "level modifier", source)?);
        }
        if held.temporary {
            self.restore_keymap();
        }
        Ok(sent)
    }

    /// Releases every held key, for the end of a run.
    pub fn release_held_keys(&mut self, source: InputSource) {
        let held: Vec<Key> = self.input.held_keys.iter().map(|h| h.key.clone()).collect();
        for key in held.into_iter().rev() {
            let _ = self.release_key(&key, source);
        }
    }

    fn plan_key(&mut self, key: &Key) -> Result<KeyPlan, InjectError> {
        let name = match key {
            Key::Code(code) => {
                return Ok(KeyPlan::Chord(Chord {
                    keycode: code + 8,
                    modifiers: Vec::new(),
                }));
            }
            Key::Name(name) => name,
        };
        let sym = keysym_by_name(name).ok_or_else(|| InjectError::UnknownKey(name.clone()))?;
        let legacy = legacy_keysym(sym);
        let canonical = legacy.unwrap_or(sym);
        if let Some(temp) = &self.input.temp
            && let Some((_, kc)) = temp.keys.iter().find(|(s, _)| *s == canonical.raw())
        {
            return Ok(KeyPlan::Chord(Chord {
                keycode: *kc,
                modifiers: Vec::new(),
            }));
        }
        let layout = self.active_layout();
        let found = std::iter::once(sym)
            .chain(legacy)
            .find_map(|s| self.input.keymap.find(s, layout));
        if let Some(chord) = found {
            return Ok(KeyPlan::Chord(chord));
        }
        let sym = canonical;
        let via_text = self.config.input.unmapped == UnmappedKeys::TextInput;
        match keysym_char(sym) {
            Some(c) if via_text && self.text_input_active() => Ok(KeyPlan::Text(c)),
            _ => Ok(KeyPlan::Temporary(sym)),
        }
    }

    /// Returns the active layout of the seat keyboard.
    fn active_layout(&mut self) -> u32 {
        let Some(keyboard) = self.seat.get_keyboard() else {
            return 0;
        };
        keyboard.with_xkb_state(self, |ctx| {
            ctx.xkb().lock().map(|x| x.active_layout().0).unwrap_or(0)
        })
    }

    /// Returns the keycode of a keysym on the temporary keymap, and switches the client keymap
    /// when the keysym is new.
    fn temporary_keycode(&mut self, sym: xkb::Keysym, key: &Key) -> Result<u32, InjectError> {
        let mut syms: Vec<u32> = self
            .input
            .temp
            .as_ref()
            .map(|t| t.keys.iter().map(|(s, _)| *s).collect())
            .unwrap_or_default();
        syms.push(sym.raw());
        let keysyms: Vec<xkb::Keysym> = syms.iter().map(|s| xkb::Keysym::new(*s)).collect();
        let (text, codes) = self
            .input
            .keymap
            .with_extra(&keysyms)
            .ok_or_else(|| InjectError::NoFreeKeycode(key.to_string()))?;
        let keyboard = self.seat.get_keyboard().ok_or(InjectError::NoKeyboard)?;
        keyboard
            .set_keymap_from_string(self, text)
            .map_err(|_| InjectError::Keymap)?;
        let keycode = *codes.last().ok_or(InjectError::Keymap)?;
        self.input.temp = Some(TempKeymap {
            keys: syms.into_iter().zip(codes).collect(),
        });
        Ok(keycode)
    }

    /// Returns to the base keymap once no temporary key is down.
    fn restore_keymap(&mut self) {
        if self.input.held_keys.iter().any(|h| h.temporary) || self.input.temp.is_none() {
            return;
        }
        self.input.temp = None;
        let text = self.input.keymap.text().to_string();
        if let Some(keyboard) = self.seat.get_keyboard()
            && let Err(e) = keyboard.set_keymap_from_string(self, text)
        {
            tracing::warn!("cannot restore the keymap: {e:?}");
        }
    }

    /// Sends one key event to the focused surface and records it.
    fn send_key(
        &mut self,
        keycode: u32,
        pressed: bool,
        detail: &str,
        source: InputSource,
    ) -> Result<SentInput, InjectError> {
        let keyboard = self.seat.get_keyboard().ok_or(InjectError::NoKeyboard)?;
        let focus = keyboard.current_focus();
        let (id, surface) = self.mark_input(focus.as_ref(), MarkKind::Key);
        keyboard.input::<(), _>(
            self,
            Keycode::new(keycode),
            if pressed {
                KeyState::Pressed
            } else {
                KeyState::Released
            },
            SERIAL_COUNTER.next_serial(),
            self.now().wayland_millis(),
            |_, _, _| FilterResult::Forward,
        );
        let mut sent = self.record_sent(vec![PendingSent {
            id,
            kind: InputKind::Key,
            surface,
            pos: None,
            detail: format!(
                "{detail} {} (keycode {keycode})",
                if pressed { "pressed" } else { "released" }
            ),
            source,
        }]);
        Ok(sent.remove(0))
    }
}
