//! The keymap of the host keyboard. A host key maps to the keysym on its base level, so the
//! session presses the key with that keysym in its own keymap, whatever layout the host uses.

use std::io::Read;
use std::os::fd::OwnedFd;

use smithay::input::keyboard::xkb;

/// The compiled host keymap.
pub struct HostKeymap {
    keymap: xkb::Keymap,
}

impl HostKeymap {
    /// Compiles a keymap from its text.
    pub fn from_text(text: String) -> Option<Self> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_string(
            &context,
            text,
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )?;
        Some(Self { keymap })
    }

    /// Reads and compiles the keymap file of `wl_keyboard.keymap`.
    pub fn from_fd(fd: OwnedFd, size: u32) -> Option<Self> {
        let mut text = String::with_capacity(size as usize);
        std::fs::File::from(fd).read_to_string(&mut text).ok()?;
        Self::from_text(text.trim_end_matches('\0').to_string())
    }

    /// Returns the keysym of an evdev key on the first layout without modifiers.
    pub fn base_keysym(&self, evdev: u32) -> Option<u32> {
        let syms = self
            .keymap
            .key_get_syms_by_level(xkb::Keycode::new(evdev + 8), 0, 0);
        (syms.len() == 1).then(|| syms[0].raw())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_base_keysyms_of_a_layout() {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let de = xkb::Keymap::new_from_names(&context, "", "", "de", "", None, 0).unwrap();
        let km = HostKeymap::from_text(de.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1)).unwrap();
        // The key right of T is Z on a German layout.
        assert_eq!(km.base_keysym(21), Some(xkb::keysyms::KEY_z));
        assert_eq!(km.base_keysym(42), Some(xkb::keysyms::KEY_Shift_L));
        assert_eq!(km.base_keysym(0), None);
    }
}
