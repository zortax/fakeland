//! RGBA colors that read `"#rrggbb"`, `"#rrggbbaa"` or `[r, g, b, a]`.

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An 8-bit RGBA color.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(try_from = "RgbaRepr", into = "RgbaRepr")]
pub struct Rgba(pub [u8; 4]);

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
enum RgbaRepr {
    Hex(String),
    Array([u8; 4]),
}

/// A color string that does not parse.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid color {0:?}; use #rrggbb or #rrggbbaa")]
pub struct RgbaParseError(String);

impl Rgba {
    /// Builds an opaque color.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }
}

impl FromStr for Rgba {
    type Err = RgbaParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let fail = || RgbaParseError(s.to_owned());
        let hex = s.trim().strip_prefix('#').ok_or_else(fail)?;
        if !(hex.len() == 6 || hex.len() == 8) || !hex.is_ascii() {
            return Err(fail());
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| fail());
        let a = if hex.len() == 8 { byte(6)? } else { 255 };
        Ok(Self([byte(0)?, byte(2)?, byte(4)?, a]))
    }
}

impl fmt::Display for Rgba {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [r, g, b, a] = self.0;
        write!(f, "#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

impl TryFrom<RgbaRepr> for Rgba {
    type Error = RgbaParseError;
    fn try_from(r: RgbaRepr) -> Result<Self, Self::Error> {
        match r {
            RgbaRepr::Hex(s) => s.parse(),
            RgbaRepr::Array(a) => Ok(Self(a)),
        }
    }
}

impl From<Rgba> for RgbaRepr {
    fn from(c: Rgba) -> Self {
        Self::Hex(c.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_forms() {
        assert_eq!("#ff8000".parse(), Ok(Rgba([255, 128, 0, 255])));
        assert_eq!("#ff800080".parse(), Ok(Rgba([255, 128, 0, 128])));
        assert!("ff8000".parse::<Rgba>().is_err());
        assert!("#ff80".parse::<Rgba>().is_err());
        assert!("#gg8000".parse::<Rgba>().is_err());
        let c: Rgba = serde_json::from_str("[1,2,3,4]").unwrap_or_default();
        assert_eq!(c, Rgba([1, 2, 3, 4]));
        let c: Rgba = serde_json::from_str("\"#010203\"").unwrap_or_default();
        assert_eq!(c, Rgba::rgb(1, 2, 3));
        assert_eq!(
            serde_json::to_string(&c).unwrap_or_default(),
            "\"#010203ff\""
        );
    }
}
