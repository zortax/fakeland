//! Key combinations such as `ctrl+shift+t`: a set of modifiers and one key.

use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::Key;

/// A modifier key.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum Modifier {
    /// Control.
    Ctrl,
    /// Shift.
    Shift,
    /// Alt (Mod1).
    Alt,
    /// Super, Logo or Meta (Mod4).
    Super,
    /// AltGr (ISO level 3 shift).
    AltGr,
}

impl Modifier {
    /// Returns the keysym name of the left modifier key.
    pub fn keysym(self) -> &'static str {
        match self {
            Self::Ctrl => "Control_L",
            Self::Shift => "Shift_L",
            Self::Alt => "Alt_L",
            Self::Super => "Super_L",
            Self::AltGr => "ISO_Level3_Shift",
        }
    }

    /// Returns the evdev code of the left modifier key.
    pub fn code(self) -> u32 {
        match self {
            Self::Ctrl => 29,
            Self::Shift => 42,
            Self::Alt => 56,
            Self::Super => 125,
            Self::AltGr => 100,
        }
    }

    /// Returns the key of the modifier.
    pub fn key(self) -> Key {
        Key::Name(self.keysym().to_owned())
    }

    /// Parses a modifier name. Accepts common aliases such as `control`, `meta`, `logo` and `cmd`.
    pub fn parse(name: &str) -> Option<Self> {
        Some(match name.to_ascii_lowercase().as_str() {
            "ctrl" | "control" | "ctl" => Self::Ctrl,
            "shift" => Self::Shift,
            "alt" | "mod1" | "option" | "opt" => Self::Alt,
            "super" | "logo" | "meta" | "win" | "cmd" | "mod4" => Self::Super,
            "altgr" | "alt_gr" => Self::AltGr,
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Ctrl => "ctrl",
            Self::Shift => "shift",
            Self::Alt => "alt",
            Self::Super => "super",
            Self::AltGr => "altgr",
        }
    }
}

/// A combination string that does not parse.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ComboError {
    /// The string is empty or has an empty part.
    #[error("key combination {0:?} has an empty part")]
    Empty(String),
    /// A modifier part names no modifier.
    #[error("{part:?} in key combination {combo:?} is not a modifier")]
    NotAModifier {
        /// The combination.
        combo: String,
        /// The part.
        part: String,
    },
    /// A `code:` part without a valid evdev code.
    #[error("{part:?} in key combination {combo:?} is not an evdev code")]
    BadCode {
        /// The combination.
        combo: String,
        /// The part.
        part: String,
    },
}

/// Modifiers and one key. The modifiers press in order before the key and release in reverse
/// order after it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Combo {
    /// The modifiers, without duplicates, in the order of the combination.
    pub modifiers: Vec<Modifier>,
    /// The key.
    pub key: Key,
}

impl Combo {
    /// Builds a combination.
    pub fn new(modifiers: impl IntoIterator<Item = Modifier>, key: Key) -> Self {
        let mut list = Vec::new();
        for m in modifiers {
            if !list.contains(&m) {
                list.push(m);
            }
        }
        Self {
            modifiers: list,
            key,
        }
    }

    /// Parses `ctrl+shift+t`. Parts split on `+`; `plus` names the plus key and `code:28` names
    /// an evdev code. A combination of modifiers only presses the last one as the key.
    pub fn parse(s: &str) -> Result<Self, ComboError> {
        let parts: Vec<&str> = s.split('+').map(str::trim).collect();
        if s.trim().is_empty() || parts.iter().any(|p| p.is_empty()) {
            return Err(ComboError::Empty(s.to_owned()));
        }
        let (last, mods) = parts
            .split_last()
            .ok_or_else(|| ComboError::Empty(s.to_owned()))?;
        let mut modifiers = Vec::new();
        for part in mods {
            let m = Modifier::parse(part).ok_or_else(|| ComboError::NotAModifier {
                combo: s.to_owned(),
                part: (*part).to_owned(),
            })?;
            modifiers.push(m);
        }
        let key = match (Modifier::parse(last), last.strip_prefix("code:")) {
            (Some(m), _) => m.key(),
            (None, Some(code)) => {
                Key::Code(code.trim().parse().map_err(|_| ComboError::BadCode {
                    combo: s.to_owned(),
                    part: (*last).to_owned(),
                })?)
            }
            (None, None) => Key::name(last),
        };
        Ok(Self::new(modifiers, key))
    }
}

impl FromStr for Combo {
    type Err = ComboError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl fmt::Display for Combo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for m in &self.modifiers {
            write!(f, "{}+", m.name())?;
        }
        match &self.key {
            Key::Name(n) if n == "plus" => f.write_str("plus"),
            Key::Name(n) => f.write_str(n),
            Key::Code(c) => write!(f, "code:{c}"),
        }
    }
}

impl Serialize for Combo {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Combo {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Self::parse(&s).map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for Combo {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Combo".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "Modifiers and a key joined by '+', for example \"ctrl+shift+t\". Modifiers: ctrl, shift, alt, super, altgr. The key is an xkb keysym name.",
            "minLength": 1
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Combo {
        Combo::parse(s).unwrap_or_else(|e| panic!("{e}"))
    }

    #[test]
    fn parses_modifiers_and_key() {
        let c = parse("ctrl+shift+t");
        assert_eq!(c.modifiers, vec![Modifier::Ctrl, Modifier::Shift]);
        assert_eq!(c.key, Key::Name("t".into()));
        assert_eq!(
            parse("Control + Alt + Delete").key,
            Key::Name("Delete".into())
        );
        assert_eq!(parse("super+enter").key, Key::Name("Return".into()));
        assert_eq!(parse("ctrl+plus").key, Key::Name("plus".into()));
        assert_eq!(parse("Return").modifiers, vec![]);
        assert_eq!(parse("ctrl+ctrl+a").modifiers, vec![Modifier::Ctrl]);
        assert_eq!(
            parse("cmd+opt+f").modifiers,
            vec![Modifier::Super, Modifier::Alt]
        );
    }

    #[test]
    fn modifier_only_combination_presses_the_last_modifier() {
        let c = parse("ctrl+shift");
        assert_eq!(c.modifiers, vec![Modifier::Ctrl]);
        assert_eq!(c.key, Key::Name("Shift_L".into()));
        assert_eq!(parse("super").key, Key::Name("Super_L".into()));
    }

    #[test]
    fn errors() {
        assert_eq!(Combo::parse(""), Err(ComboError::Empty(String::new())));
        assert!(matches!(Combo::parse("ctrl++"), Err(ComboError::Empty(_))));
        assert!(matches!(
            Combo::parse("hyper+x"),
            Err(ComboError::NotAModifier { .. })
        ));
    }

    #[test]
    fn display_and_serde_round_trip() {
        assert_eq!(parse("ctrl+code:28").key, Key::Code(28));
        assert!(matches!(
            Combo::parse("ctrl+code:x"),
            Err(ComboError::BadCode { .. })
        ));
        for s in [
            "ctrl+shift+t",
            "alt+F4",
            "super+Return",
            "ctrl+plus",
            "ctrl+period",
            "code:30",
        ] {
            let c = parse(s);
            assert_eq!(parse(&c.to_string()), c);
            let json = serde_json::to_string(&c).unwrap_or_default();
            let back: Combo = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{e}"));
            assert_eq!(back, c);
        }
        assert_eq!(parse("ctrl+shift+t").to_string(), "ctrl+shift+t");
        assert!(serde_json::from_str::<Combo>("\"hyper+x\"").is_err());
    }
}
