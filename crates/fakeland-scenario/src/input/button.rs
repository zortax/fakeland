//! Pointer buttons by name or evdev code.

use std::fmt;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A pointer button. JSON accepts a name (`"left"`) or an evdev code (`272`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(from = "ButtonRepr", into = "ButtonRepr")]
pub enum Button {
    /// `BTN_LEFT`.
    #[default]
    Left,
    /// `BTN_RIGHT`.
    Right,
    /// `BTN_MIDDLE`.
    Middle,
    /// `BTN_SIDE`.
    Side,
    /// `BTN_EXTRA`.
    Extra,
    /// `BTN_FORWARD`.
    Forward,
    /// `BTN_BACK`.
    Back,
    /// Any other evdev button code.
    Code(u32),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum ButtonRepr {
    Name(ButtonName),
    Code(u32),
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
enum ButtonName {
    Left,
    Right,
    Middle,
    Side,
    Extra,
    Forward,
    Back,
}

const NAMED: [(Button, ButtonName, u32); 7] = [
    (Button::Left, ButtonName::Left, 0x110),
    (Button::Right, ButtonName::Right, 0x111),
    (Button::Middle, ButtonName::Middle, 0x112),
    (Button::Side, ButtonName::Side, 0x113),
    (Button::Extra, ButtonName::Extra, 0x114),
    (Button::Forward, ButtonName::Forward, 0x115),
    (Button::Back, ButtonName::Back, 0x116),
];

impl Button {
    /// Returns the evdev code.
    pub fn code(self) -> u32 {
        match self {
            Self::Code(c) => c,
            named => NAMED
                .iter()
                .find(|(b, _, _)| *b == named)
                .map_or(0x110, |(_, _, c)| *c),
        }
    }

    /// Returns the button for an evdev code, with a name where one exists.
    pub fn from_code(code: u32) -> Self {
        NAMED
            .iter()
            .find(|(_, _, c)| *c == code)
            .map_or(Self::Code(code), |(b, _, _)| *b)
    }
}

impl From<ButtonRepr> for Button {
    fn from(r: ButtonRepr) -> Self {
        match r {
            ButtonRepr::Code(c) => Self::from_code(c),
            ButtonRepr::Name(n) => NAMED
                .iter()
                .find(|(_, name, _)| std::mem::discriminant(name) == std::mem::discriminant(&n))
                .map_or(Self::Left, |(b, _, _)| *b),
        }
    }
}

impl From<Button> for ButtonRepr {
    fn from(b: Button) -> Self {
        NAMED
            .iter()
            .find(|(named, _, _)| *named == b)
            .map_or(Self::Code(b.code()), |(_, name, _)| Self::Name(*name))
    }
}

impl fmt::Display for Button {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Code(c) => write!(f, "button {c:#x}"),
            other => write!(f, "{other:?}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes() {
        assert_eq!(Button::Left.code(), 272);
        assert_eq!(Button::Back.code(), 278);
        assert_eq!(Button::from_code(273), Button::Right);
        assert_eq!(Button::from_code(300), Button::Code(300));
    }

    #[test]
    fn serde_forms() {
        let b: Button = serde_json::from_str("\"middle\"").unwrap_or_default();
        assert_eq!(b, Button::Middle);
        let b: Button = serde_json::from_str("273").unwrap_or_default();
        assert_eq!(b, Button::Right);
        assert_eq!(
            serde_json::to_string(&Button::Code(300)).unwrap_or_default(),
            "300"
        );
        assert_eq!(
            serde_json::to_string(&Button::Side).unwrap_or_default(),
            "\"side\""
        );
    }
}
