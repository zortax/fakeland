//! The xkb keyboard state from the wl_keyboard keymap: keysyms and text for key codes, and
//! modifier updates.

use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;

use xkbcommon::xkb;

/// A keymap that cannot be used.
#[derive(Debug, thiserror::Error)]
pub enum KeymapError {
    /// The keymap fd could not be read.
    #[error("cannot read the keymap: {0}")]
    Read(#[from] std::io::Error),
    /// The keymap is not UTF-8 text.
    #[error("the keymap is not UTF-8 text")]
    Encoding,
    /// xkbcommon rejected the keymap.
    #[error("xkbcommon rejected the keymap")]
    Compile,
}

/// The compiled keymap and its state.
pub struct KeyState {
    state: xkb::State,
}

impl std::fmt::Debug for KeyState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KeyState")
    }
}

impl KeyState {
    /// Compiles a keymap from its text.
    pub fn from_text(text: String) -> Result<Self, KeymapError> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_string(
            &context,
            text,
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or(KeymapError::Compile)?;
        Ok(Self {
            state: xkb::State::new(&keymap),
        })
    }

    /// Reads and compiles the keymap that wl_keyboard.keymap sends.
    pub fn from_fd(fd: OwnedFd, size: u32) -> Result<Self, KeymapError> {
        let mut bytes = Vec::with_capacity(size as usize);
        File::from(fd).take(size as u64).read_to_end(&mut bytes)?;
        while bytes.last() == Some(&0) {
            bytes.pop();
        }
        let text = String::from_utf8(bytes).map_err(|_| KeymapError::Encoding)?;
        Self::from_text(text)
    }

    /// Returns the keysym and the text of an evdev key code in the current state.
    pub fn key(&self, code: u32) -> (u32, String) {
        let keycode = xkb::Keycode::new(code + 8);
        (
            self.state.key_get_one_sym(keycode).raw(),
            self.state.key_get_utf8(keycode),
        )
    }

    /// Applies wl_keyboard.modifiers.
    pub fn update_mask(&mut self, depressed: u32, latched: u32, locked: u32, group: u32) {
        self.state
            .update_mask(depressed, latched, locked, 0, 0, group);
    }
}

/// Returns the xkb name of a keysym.
pub fn keysym_name(keysym: u32) -> String {
    xkb::keysym_get_name(xkb::Keysym::new(keysym))
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEYMAP: &str = r#"xkb_keymap {
        xkb_keycodes "t" { minimum = 8; maximum = 255; <AC01> = 38; <LFSH> = 50; <RTRN> = 36; };
        xkb_types "t" { include "complete" };
        xkb_compatibility "t" { include "complete" };
        xkb_symbols "t" {
            key <AC01> { [ a, A ] };
            key <LFSH> { [ Shift_L ] };
            key <RTRN> { [ Return ] };
            modifier_map Shift { <LFSH> };
        };
    };"#;

    #[test]
    fn keys_and_modifiers() {
        let mut k = match KeyState::from_text(KEYMAP.to_owned()) {
            Ok(k) => k,
            // The includes need the system xkb data; the test skips only without it.
            Err(KeymapError::Compile) if !std::path::Path::new("/usr/share/X11/xkb").exists() => {
                return;
            }
            Err(e) => panic!("{e}"),
        };
        // Evdev code 30 is xkb keycode 38.
        assert_eq!(k.key(30), (0x61, "a".to_owned()));
        k.update_mask(1, 0, 0, 0);
        assert_eq!(k.key(30), (0x41, "A".to_owned()));
        assert_eq!(k.key(28).0, 0xff0d);
        assert_eq!(keysym_name(0xff0d), "Return");
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(matches!(
            KeyState::from_text("not a keymap".to_owned()),
            Err(KeymapError::Compile)
        ));
    }
}
