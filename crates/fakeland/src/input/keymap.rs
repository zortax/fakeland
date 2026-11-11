//! Keysym lookup in an xkb keymap. A keysym resolves to a keycode and the modifier keys that
//! select its shift level. A keymap with extra keysyms on free keycodes serves keysyms that the
//! layout lacks.

use smithay::input::keyboard::xkb;
use thiserror::Error;

/// A keymap that does not compile.
#[derive(Debug, Error)]
pub enum KeymapError {
    /// The rules, model, layout, variant and options do not compile.
    #[error("the keymap {0:?} does not compile")]
    Names(String),
}

/// A key and the modifier keys that select the level of a keysym. Codes are xkb keycodes, which
/// are evdev codes plus 8.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chord {
    /// The key.
    pub keycode: u32,
    /// The modifier keys to hold, in press order.
    pub modifiers: Vec<u32>,
}

/// A compiled keymap with its text.
pub struct KeymapLookup {
    keymap: xkb::Keymap,
    text: String,
}

impl std::fmt::Debug for KeymapLookup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeymapLookup")
            .field("bytes", &self.text.len())
            .finish()
    }
}

/// The modifier keysyms that select shift levels, in the order they combine.
const LEVEL_KEYSYMS: [u32; 2] = [
    xkb::keysyms::KEY_Shift_L,
    xkb::keysyms::KEY_ISO_Level3_Shift,
];

impl KeymapLookup {
    /// Compiles a keymap from rules, model, layout, variant and options.
    pub fn from_names(
        rules: &str,
        model: &str,
        layout: &str,
        variant: &str,
        options: Option<String>,
    ) -> Result<Self, KeymapError> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_names(
            &context,
            rules,
            model,
            layout,
            variant,
            options,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .ok_or_else(|| KeymapError::Names(layout.to_string()))?;
        let text = keymap.get_as_string(xkb::KEYMAP_FORMAT_TEXT_V1);
        Ok(Self { keymap, text })
    }

    /// Compiles a keymap from its text form.
    #[cfg(test)]
    pub fn from_text(text: String) -> Option<Self> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
        let keymap = xkb::Keymap::new_from_string(
            &context,
            text.clone(),
            xkb::KEYMAP_FORMAT_TEXT_V1,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        )?;
        Some(Self { keymap, text })
    }

    /// Returns the keymap text.
    pub fn text(&self) -> &str {
        &self.text
    }

    fn keycodes(&self) -> impl DoubleEndedIterator<Item = xkb::Keycode> {
        let min = self.keymap.min_keycode().raw();
        let max = self.keymap.max_keycode().raw();
        (min..=max).map(xkb::Keycode::new)
    }

    /// Returns the key that produces `keysym`, with the modifier keys that select its level in
    /// `layout`. Keycodes up to 255 come first, then lower levels, then lower keycodes.
    pub fn find(&self, keysym: xkb::Keysym, layout: u32) -> Option<Chord> {
        let mut best: Option<((bool, u32, u32), xkb::Keycode, u32)> = None;
        for kc in self.keycodes() {
            if self.keymap.num_layouts_for_key(kc) == 0 {
                continue;
            }
            let levels = self.keymap.num_levels_for_key(kc, layout);
            for level in 0..levels {
                let syms = self.keymap.key_get_syms_by_level(kc, layout, level);
                if syms.len() == 1 && syms[0] == keysym {
                    let rank = (kc.raw() > 255, level, kc.raw());
                    if best.as_ref().is_none_or(|(r, _, _)| rank < *r) {
                        best = Some((rank, kc, level));
                    }
                    break;
                }
            }
        }
        let (_, kc, level) = best?;
        if level == 0 {
            return Some(Chord {
                keycode: kc.raw(),
                modifiers: Vec::new(),
            });
        }
        self.modifiers_for(kc, keysym, layout)
            .map(|modifiers| Chord {
                keycode: kc.raw(),
                modifiers,
            })
    }

    /// Returns the modifier keys that make `kc` produce `keysym`. Every subset of the level
    /// modifiers is tried on a fresh state, smallest first.
    fn modifiers_for(
        &self,
        kc: xkb::Keycode,
        keysym: xkb::Keysym,
        layout: u32,
    ) -> Option<Vec<u32>> {
        let mods: Vec<u32> = LEVEL_KEYSYMS
            .iter()
            .filter_map(|&sym| self.base_keycode(xkb::Keysym::new(sym)))
            .collect();
        let subsets: Vec<Vec<u32>> = (1u32..(1 << mods.len()))
            .map(|mask| {
                mods.iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, m)| *m)
                    .collect()
            })
            .collect();
        let mut subsets = subsets;
        subsets.sort_by_key(Vec::len);
        subsets.into_iter().find(|set| {
            let mut state = xkb::State::new(&self.keymap);
            state.update_mask(0, 0, 0, 0, 0, layout);
            for m in set {
                state.update_key(xkb::Keycode::new(*m), xkb::KeyDirection::Down);
            }
            state.key_get_one_sym(kc) == keysym
        })
    }

    /// Returns the key that produces `keysym` without modifiers in any layout.
    pub fn base_keycode(&self, keysym: xkb::Keysym) -> Option<u32> {
        self.keycodes().map(|kc| kc.raw()).find(|&kc| {
            let kc = xkb::Keycode::new(kc);
            (0..self.keymap.num_layouts_for_key(kc)).any(|layout| {
                let syms = self.keymap.key_get_syms_by_level(kc, layout, 0);
                syms.len() == 1 && syms[0] == keysym
            })
        })
    }

    /// Returns a keymap text with each keysym on a free keycode, and the keycodes in the order of
    /// `keysyms`. A free keycode has a name and no symbols; the highest free keycodes up to 255
    /// come first. Returns `None` when the keymap runs out of free keycodes.
    pub fn with_extra(&self, keysyms: &[xkb::Keysym]) -> Option<(String, Vec<u32>)> {
        let mut free: Vec<(u32, String)> = self
            .keycodes()
            .filter(|kc| self.keymap.num_layouts_for_key(*kc) == 0)
            .filter_map(|kc| {
                let name = self.keymap.key_get_name(kc)?;
                Some((kc.raw(), name.to_string()))
            })
            .collect();
        free.sort_by_key(|(kc, _)| (*kc > 255, std::cmp::Reverse(*kc)));
        free.truncate(keysyms.len());
        if free.len() < keysyms.len() {
            return None;
        }
        let lines: String = free
            .iter()
            .zip(keysyms)
            .map(|((_, name), sym)| {
                format!("\tkey <{name}> {{ [ {} ] }};\n", xkb::keysym_get_name(*sym))
            })
            .collect();
        let text = insert_symbols(&self.text, &lines)?;
        Some((text, free.iter().map(|(kc, _)| *kc).collect()))
    }
}

/// Inserts symbol lines at the end of the `xkb_symbols` section of a keymap text.
pub fn insert_symbols(text: &str, lines: &str) -> Option<String> {
    let start = text.find("xkb_symbols")?;
    let end = start + text[start..].find("\n};")?;
    let mut out = String::with_capacity(text.len() + lines.len());
    out.push_str(&text[..=end]);
    out.push_str(lines);
    out.push_str(&text[end + 1..]);
    Some(out)
}

/// Returns the legacy keysym of a Unicode keysym, such as `EuroSign` for `U20AC`. Keymaps name
/// most characters by their legacy keysym.
pub fn legacy_keysym(sym: xkb::Keysym) -> Option<xkb::Keysym> {
    let cp = xkb::keysym_to_utf32(sym);
    if cp == 0 {
        return None;
    }
    let legacy = xkb::utf32_to_keysym(cp);
    (legacy != sym && legacy.raw() != xkb::keysyms::KEY_NoSymbol).then_some(legacy)
}

/// Resolves a keysym name. Names are case sensitive first, then case insensitive.
pub fn keysym_by_name(name: &str) -> Option<xkb::Keysym> {
    let sym = xkb::keysym_from_name(name, xkb::KEYSYM_NO_FLAGS);
    if sym.raw() != xkb::keysyms::KEY_NoSymbol {
        return Some(sym);
    }
    let sym = xkb::keysym_from_name(name, xkb::KEYSYM_CASE_INSENSITIVE);
    (sym.raw() != xkb::keysyms::KEY_NoSymbol).then_some(sym)
}

/// Returns the character a keysym types, if any.
pub fn keysym_char(sym: xkb::Keysym) -> Option<char> {
    char::from_u32(xkb::keysym_to_utf32(sym)).filter(|c| *c != '\0' && !c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn us() -> KeymapLookup {
        KeymapLookup::from_names("", "", "us", "", None).unwrap()
    }

    fn sym(name: &str) -> xkb::Keysym {
        keysym_by_name(name).unwrap()
    }

    #[test]
    fn finds_base_and_shifted_keys() {
        let km = us();
        assert_eq!(
            km.find(sym("a"), 0),
            Some(Chord {
                keycode: 38,
                modifiers: vec![]
            })
        );
        assert_eq!(
            km.find(sym("A"), 0),
            Some(Chord {
                keycode: 38,
                modifiers: vec![50]
            })
        );
        assert_eq!(km.find(sym("exclam"), 0).unwrap().keycode, 10);
        assert_eq!(km.find(sym("Return"), 0).unwrap().keycode, 36);
        assert_eq!(km.find(sym("space"), 0).unwrap().keycode, 65);
        assert_eq!(km.find(sym("Control_L"), 0).unwrap().keycode, 37);
        assert_eq!(km.find(sym("U263A"), 0), None);
        assert_eq!(km.find(sym("Greek_alpha"), 0), None);
    }

    #[test]
    fn finds_level3_keys() {
        let km = KeymapLookup::from_names("", "", "de", "", None).unwrap();
        let euro = km.find(sym("EuroSign"), 0).unwrap();
        assert_eq!(euro.keycode, 26);
        assert_eq!(euro.modifiers.len(), 1);
        let at = km.find(sym("at"), 0).unwrap();
        assert_eq!(at.keycode, 24);
        assert_eq!(euro.modifiers, at.modifiers);
    }

    #[test]
    fn extra_keysyms_land_on_free_keycodes() {
        let km = us();
        let wanted = [sym("Greek_alpha"), sym("U263A")];
        let (text, codes) = km.with_extra(&wanted).unwrap();
        assert_eq!(codes.len(), 2);
        assert!(codes.iter().all(|c| *c <= 255));
        let extended = KeymapLookup::from_text(text).unwrap();
        for (code, s) in codes.iter().zip(wanted) {
            assert_eq!(
                extended.find(s, 0),
                Some(Chord {
                    keycode: *code,
                    modifiers: vec![]
                })
            );
        }
        assert_eq!(extended.find(sym("a"), 0).unwrap().keycode, 38);
    }

    #[test]
    fn names_and_characters() {
        assert_eq!(keysym_by_name("return"), Some(sym("Return")));
        assert_eq!(keysym_by_name("not-a-keysym"), None);
        assert_eq!(keysym_char(sym("U20AC")), Some('€'));
        assert_eq!(keysym_char(sym("a")), Some('a'));
        assert_eq!(keysym_char(sym("Return")), None);
        assert_eq!(legacy_keysym(sym("U20AC")), Some(sym("EuroSign")));
        assert_eq!(sym("U00E4"), sym("adiaeresis"));
        assert_eq!(legacy_keysym(sym("U00E4")), None);
        assert_eq!(legacy_keysym(sym("a")), None);
    }

    #[test]
    fn inserts_at_the_end_of_the_symbols_section() {
        let text = "xkb_keymap {\nxkb_types \"t\" {\n};\n\nxkb_symbols \"s\" {\n\tkey <A> { [ a ] };\n};\n\n};\n";
        let out = insert_symbols(text, "\tkey <B> { [ b ] };\n").unwrap();
        assert!(out.contains("\tkey <A> { [ a ] };\n\tkey <B> { [ b ] };\n};\n\n};"));
        assert_eq!(insert_symbols("no sections", "x"), None);
    }
}
