//! DRM fourcc pixel formats of frame readbacks and their conversion to RGBA8.
//!
//! DRM formats name the channels of a little-endian word from the high bits to the low bits.
//! `ARGB8888` stores the bytes blue, green, red, alpha in memory. A `GL_RGBA` readback has the
//! layout of `ABGR8888`. Ten-bit channels lose their two low bits. A two-bit alpha expands to the
//! full eight-bit range. An `X` channel becomes opaque alpha.

use serde::{Deserialize, Serialize};

use super::ImageError;

/// Builds a DRM fourcc code from its four characters.
pub const fn fourcc(code: &[u8; 4]) -> u32 {
    code[0] as u32 | (code[1] as u32) << 8 | (code[2] as u32) << 16 | (code[3] as u32) << 24
}

/// A readback pixel format with four bytes per pixel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PixelFormat {
    /// `AR24`: bytes blue, green, red, alpha.
    Argb8888,
    /// `XR24`: bytes blue, green, red, unused.
    Xrgb8888,
    /// `AB24`: bytes red, green, blue, alpha.
    Abgr8888,
    /// `XB24`: bytes red, green, blue, unused.
    Xbgr8888,
    /// `AR30`: a 32-bit word with 2 bits alpha, 10 bits red, green and blue.
    Argb2101010,
    /// `XR30`: a 32-bit word with 2 unused bits, 10 bits red, green and blue.
    Xrgb2101010,
    /// `AB30`: a 32-bit word with 2 bits alpha, 10 bits blue, green and red.
    Abgr2101010,
    /// `XB30`: a 32-bit word with 2 unused bits, 10 bits blue, green and red.
    Xbgr2101010,
}

impl PixelFormat {
    /// Every supported format.
    pub const ALL: [PixelFormat; 8] = [
        PixelFormat::Argb8888,
        PixelFormat::Xrgb8888,
        PixelFormat::Abgr8888,
        PixelFormat::Xbgr8888,
        PixelFormat::Argb2101010,
        PixelFormat::Xrgb2101010,
        PixelFormat::Abgr2101010,
        PixelFormat::Xbgr2101010,
    ];

    /// Returns the DRM fourcc code.
    pub const fn fourcc(self) -> u32 {
        match self {
            PixelFormat::Argb8888 => fourcc(b"AR24"),
            PixelFormat::Xrgb8888 => fourcc(b"XR24"),
            PixelFormat::Abgr8888 => fourcc(b"AB24"),
            PixelFormat::Xbgr8888 => fourcc(b"XB24"),
            PixelFormat::Argb2101010 => fourcc(b"AR30"),
            PixelFormat::Xrgb2101010 => fourcc(b"XR30"),
            PixelFormat::Abgr2101010 => fourcc(b"AB30"),
            PixelFormat::Xbgr2101010 => fourcc(b"XB30"),
        }
    }

    /// Returns the format of a DRM fourcc code.
    pub fn from_fourcc(code: u32) -> Result<Self, ImageError> {
        Self::ALL
            .into_iter()
            .find(|f| f.fourcc() == code)
            .ok_or(ImageError::UnsupportedFourcc(code))
    }

    /// Returns the bytes per pixel.
    pub const fn bytes_per_pixel(self) -> usize {
        4
    }

    /// Returns true when the format carries an alpha channel.
    pub const fn has_alpha(self) -> bool {
        matches!(
            self,
            PixelFormat::Argb8888
                | PixelFormat::Abgr8888
                | PixelFormat::Argb2101010
                | PixelFormat::Abgr2101010
        )
    }

    /// Converts one row of pixels into RGBA8. `src` and `dst` hold the same pixel count.
    pub(crate) fn convert_row(self, src: &[u8], dst: &mut [u8]) {
        let pixels = src.chunks_exact(4).zip(dst.chunks_exact_mut(4));
        match self {
            PixelFormat::Abgr8888 => dst.copy_from_slice(src),
            PixelFormat::Xbgr8888 => {
                for (s, d) in pixels {
                    d.copy_from_slice(&[s[0], s[1], s[2], 255]);
                }
            }
            PixelFormat::Argb8888 => {
                for (s, d) in pixels {
                    d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
                }
            }
            PixelFormat::Xrgb8888 => {
                for (s, d) in pixels {
                    d.copy_from_slice(&[s[2], s[1], s[0], 255]);
                }
            }
            PixelFormat::Argb2101010 | PixelFormat::Xrgb2101010 => {
                let alpha = self.has_alpha();
                for (s, d) in pixels {
                    let w = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                    d.copy_from_slice(&[
                        ten_to_eight(w >> 20),
                        ten_to_eight(w >> 10),
                        ten_to_eight(w),
                        two_bit_alpha(w, alpha),
                    ]);
                }
            }
            PixelFormat::Abgr2101010 | PixelFormat::Xbgr2101010 => {
                let alpha = self.has_alpha();
                for (s, d) in pixels {
                    let w = u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
                    d.copy_from_slice(&[
                        ten_to_eight(w),
                        ten_to_eight(w >> 10),
                        ten_to_eight(w >> 20),
                        two_bit_alpha(w, alpha),
                    ]);
                }
            }
        }
    }
}

/// Keeps the high eight bits of the low ten bits.
fn ten_to_eight(v: u32) -> u8 {
    ((v & 0x3ff) >> 2) as u8
}

/// Expands the top two bits to eight bits, or returns opaque alpha.
fn two_bit_alpha(w: u32, has_alpha: bool) -> u8 {
    if has_alpha {
        ((w >> 30) * 85) as u8
    } else {
        255
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(format: PixelFormat, px: [u8; 4]) -> [u8; 4] {
        let mut out = [0u8; 4];
        format.convert_row(&px, &mut out);
        out
    }

    fn word10(hi: u32, a: u32, b: u32, c: u32) -> [u8; 4] {
        (hi << 30 | a << 20 | b << 10 | c).to_le_bytes()
    }

    #[test]
    fn fourcc_round_trip() {
        assert_eq!(PixelFormat::Argb8888.fourcc(), 0x3432_5241);
        assert_eq!(PixelFormat::Xbgr2101010.fourcc(), 0x3033_4258);
        for f in PixelFormat::ALL {
            assert_eq!(PixelFormat::from_fourcc(f.fourcc()).ok(), Some(f));
        }
        assert!(matches!(
            PixelFormat::from_fourcc(fourcc(b"NV12")),
            Err(ImageError::UnsupportedFourcc(_))
        ));
    }

    #[test]
    fn eight_bit_orders() {
        let px = [10, 20, 30, 40];
        assert_eq!(convert(PixelFormat::Argb8888, px), [30, 20, 10, 40]);
        assert_eq!(convert(PixelFormat::Xrgb8888, px), [30, 20, 10, 255]);
        assert_eq!(convert(PixelFormat::Abgr8888, px), [10, 20, 30, 40]);
        assert_eq!(convert(PixelFormat::Xbgr8888, px), [10, 20, 30, 255]);
    }

    #[test]
    fn ten_bit_orders_truncate() {
        let px = word10(2, 1023, 512, 7);
        assert_eq!(convert(PixelFormat::Argb2101010, px), [255, 128, 1, 170]);
        assert_eq!(convert(PixelFormat::Xrgb2101010, px), [255, 128, 1, 255]);
        assert_eq!(convert(PixelFormat::Abgr2101010, px), [1, 128, 255, 170]);
        assert_eq!(convert(PixelFormat::Xbgr2101010, px), [1, 128, 255, 255]);
        assert_eq!(
            convert(PixelFormat::Argb2101010, word10(3, 0, 0, 0))[3],
            255
        );
    }
}
