//! Keys by xkb keysym name or evdev code, and the keysym names of characters.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A key. JSON accepts a keysym name (`"Return"`, `"a"`, aliases such as `"enter"`) or an evdev
/// code (`28`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(from = "KeyRepr", into = "KeyRepr")]
pub enum Key {
    /// An xkb keysym name. The compositor finds the key and the levels in the active keymap.
    Name(String),
    /// An evdev key code.
    Code(u32),
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum KeyRepr {
    Name(String),
    Code(u32),
}

impl From<KeyRepr> for Key {
    fn from(r: KeyRepr) -> Self {
        match r {
            KeyRepr::Name(n) => Self::name(&n),
            KeyRepr::Code(c) => Self::Code(c),
        }
    }
}

impl From<Key> for KeyRepr {
    fn from(k: Key) -> Self {
        match k {
            Key::Name(n) => Self::Name(n),
            Key::Code(c) => Self::Code(c),
        }
    }
}

impl Key {
    /// Builds a key from a keysym name. Common aliases map to xkb names, for example `enter`
    /// becomes `Return`, `esc` becomes `Escape` and `ctrl` becomes `Control_L`.
    pub fn name(name: &str) -> Self {
        Self::Name(canonical_name(name))
    }

    /// Builds a key from an evdev code.
    pub fn code(code: u32) -> Self {
        Self::Code(code)
    }

    /// Builds the key that types a character.
    pub fn char(c: char) -> Self {
        Self::Name(keysym_for_char(c))
    }
}

impl From<&str> for Key {
    fn from(s: &str) -> Self {
        Self::name(s)
    }
}

impl From<char> for Key {
    fn from(c: char) -> Self {
        Self::char(c)
    }
}

impl fmt::Display for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Name(n) => f.write_str(n),
            Self::Code(c) => write!(f, "keycode {c}"),
        }
    }
}

const ALIASES: &[(&str, &str)] = &[
    ("enter", "Return"),
    ("return", "Return"),
    ("ret", "Return"),
    ("esc", "Escape"),
    ("escape", "Escape"),
    ("tab", "Tab"),
    ("space", "space"),
    ("spc", "space"),
    ("backspace", "BackSpace"),
    ("bksp", "BackSpace"),
    ("delete", "Delete"),
    ("del", "Delete"),
    ("insert", "Insert"),
    ("ins", "Insert"),
    ("home", "Home"),
    ("end", "End"),
    ("pageup", "Prior"),
    ("pgup", "Prior"),
    ("pagedown", "Next"),
    ("pgdn", "Next"),
    ("up", "Up"),
    ("down", "Down"),
    ("left", "Left"),
    ("right", "Right"),
    ("menu", "Menu"),
    ("print", "Print"),
    ("capslock", "Caps_Lock"),
    ("plus", "plus"),
    ("minus", "minus"),
];

fn canonical_name(name: &str) -> String {
    if let Some(m) = crate::Modifier::parse(name) {
        return m.keysym().to_owned();
    }
    let lower = name.to_ascii_lowercase();
    if let Some((_, canon)) = ALIASES.iter().find(|(alias, _)| *alias == lower) {
        return (*canon).to_owned();
    }
    if let Some(n) = lower.strip_prefix('f')
        && let Ok(n) = n.parse::<u8>()
        && (1..=35).contains(&n)
    {
        return format!("F{n}");
    }
    let mut chars = name.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        return keysym_for_char(c);
    }
    name.to_owned()
}

const PUNCTUATION: &[(char, &str)] = &[
    (' ', "space"),
    ('!', "exclam"),
    ('"', "quotedbl"),
    ('#', "numbersign"),
    ('$', "dollar"),
    ('%', "percent"),
    ('&', "ampersand"),
    ('\'', "apostrophe"),
    ('(', "parenleft"),
    (')', "parenright"),
    ('*', "asterisk"),
    ('+', "plus"),
    (',', "comma"),
    ('-', "minus"),
    ('.', "period"),
    ('/', "slash"),
    (':', "colon"),
    (';', "semicolon"),
    ('<', "less"),
    ('=', "equal"),
    ('>', "greater"),
    ('?', "question"),
    ('@', "at"),
    ('[', "bracketleft"),
    ('\\', "backslash"),
    (']', "bracketright"),
    ('^', "asciicircum"),
    ('_', "underscore"),
    ('`', "grave"),
    ('{', "braceleft"),
    ('|', "bar"),
    ('}', "braceright"),
    ('~', "asciitilde"),
    ('\n', "Return"),
    ('\r', "Return"),
    ('\t', "Tab"),
    ('\u{8}', "BackSpace"),
    ('\u{1b}', "Escape"),
];

/// Returns the xkb keysym name that types a character: letters and digits are their own names,
/// ASCII punctuation uses xkb names, and other characters use the `U<hex>` form.
pub fn keysym_for_char(c: char) -> String {
    if c.is_ascii_alphanumeric() {
        return c.to_string();
    }
    if let Some((_, name)) = PUNCTUATION.iter().find(|(p, _)| *p == c) {
        return (*name).to_owned();
    }
    format!("U{:04X}", c as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_aliases() {
        assert_eq!(Key::name("enter"), Key::Name("Return".into()));
        assert_eq!(Key::name("Esc"), Key::Name("Escape".into()));
        assert_eq!(Key::name("f5"), Key::Name("F5".into()));
        assert_eq!(Key::name("a"), Key::Name("a".into()));
        assert_eq!(Key::name("."), Key::Name("period".into()));
        assert_eq!(
            Key::name("XF86AudioMute"),
            Key::Name("XF86AudioMute".into())
        );
        assert_eq!(Key::name("PageDown"), Key::Name("Next".into()));
        assert_eq!(Key::name("ctrl"), Key::Name("Control_L".into()));
        assert_eq!(Key::name("Super_L"), Key::Name("Super_L".into()));
    }

    #[test]
    fn characters() {
        assert_eq!(keysym_for_char('A'), "A");
        assert_eq!(keysym_for_char('7'), "7");
        assert_eq!(keysym_for_char(' '), "space");
        assert_eq!(keysym_for_char('\n'), "Return");
        assert_eq!(keysym_for_char('€'), "U20AC");
        assert_eq!(keysym_for_char('ä'), "U00E4");
    }

    #[test]
    fn serde_forms() {
        let k: Key = serde_json::from_str("\"Return\"").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(k, Key::Name("Return".into()));
        let k: Key = serde_json::from_str("\"esc\"").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(k, Key::Name("Escape".into()));
        let k: Key = serde_json::from_str("28").unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(k, Key::Code(28));
    }
}
