//! Opaque RGB colors, their text form and their XRGB8888 pixel bytes.

use std::fmt;
use std::str::FromStr;

/// An opaque color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

/// A color string that is not `#rrggbb`.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid color {0:?}: use #rrggbb")]
pub struct ColorError(pub String);

impl Color {
    /// Black.
    #[cfg(test)]
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    /// White.
    pub const WHITE: Self = Self::rgb(255, 255, 255);

    /// Builds a color from its channels.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Returns the XRGB8888 pixel in memory order: blue, green, red, then an opaque X byte.
    pub const fn pixel(self) -> [u8; 4] {
        [self.b, self.g, self.r, 0xff]
    }

    /// Reads a color back from XRGB8888 pixel bytes.
    #[cfg(test)]
    pub const fn from_pixel(p: [u8; 4]) -> Self {
        Self::rgb(p[2], p[1], p[0])
    }
}

impl FromStr for Color {
    type Err = ColorError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex = s.strip_prefix('#').unwrap_or(s);
        let bad = || ColorError(s.to_owned());
        if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(bad());
        }
        let ch = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|_| bad());
        Ok(Self::rgb(ch(0)?, ch(2)?, ch(4)?))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints() {
        let c: Color = "#ff8000".parse().unwrap();
        assert_eq!(c, Color::rgb(255, 128, 0));
        assert_eq!(c.to_string(), "#ff8000");
        assert_eq!("00ff00".parse::<Color>().unwrap(), Color::rgb(0, 255, 0));
    }

    #[test]
    fn rejects_garbage() {
        assert!("#fff".parse::<Color>().is_err());
        assert!("#gg0000".parse::<Color>().is_err());
        assert!("".parse::<Color>().is_err());
    }

    #[test]
    fn pixel_is_xrgb_little_endian() {
        let c = Color::rgb(1, 2, 3);
        assert_eq!(c.pixel(), [3, 2, 1, 0xff]);
        assert_eq!(Color::from_pixel(c.pixel()), c);
    }
}
