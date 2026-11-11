//! Key names in injected key event details.

/// Key names that change the level or state of other keys and draw no glyph.
const MODIFIERS: &[&str] = &[
    "level modifier",
    "Shift_L",
    "Shift_R",
    "Control_L",
    "Control_R",
    "Alt_L",
    "Alt_R",
    "Super_L",
    "Super_R",
    "Meta_L",
    "Meta_R",
    "ISO_Level3_Shift",
    "ISO_Level5_Shift",
    "Caps_Lock",
    "Num_Lock",
];

/// Returns the key name and the press state of a key detail such as `a pressed (keycode 38)`.
pub fn key_name(detail: &str) -> Option<(&str, bool)> {
    if let Some(i) = detail.rfind(" pressed") {
        return Some((&detail[..i], true));
    }
    detail.rfind(" released").map(|i| (&detail[..i], false))
}

/// Returns true for a modifier key.
pub fn is_modifier(name: &str) -> bool {
    MODIFIERS.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_details() {
        assert_eq!(key_name("a pressed (keycode 38)"), Some(("a", true)));
        assert_eq!(
            key_name("level modifier released (keycode 50)"),
            Some(("level modifier", false))
        );
        assert_eq!(key_name("garbage"), None);
        assert!(is_modifier("Shift_L") && !is_modifier("a"));
    }
}
