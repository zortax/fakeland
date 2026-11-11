//! Conversion of raw frame readbacks into RGBA8 images.

use super::view::{BPP, validate};
use super::{Image, ImageError, PixelFormat};

/// The row order of a readback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RowOrder {
    /// The first row in memory is the top row.
    #[default]
    TopDown,
    /// The first row in memory is the bottom row, as in a GL readback.
    BottomUp,
}

impl Image {
    /// Converts a raw readback into an RGBA8 image. `stride` is the source row stride in bytes.
    /// `BottomUp` rows are flipped so the result is top down.
    pub fn from_readback(
        data: &[u8],
        width: u32,
        height: u32,
        stride: usize,
        format: PixelFormat,
        order: RowOrder,
    ) -> Result<Image, ImageError> {
        validate(width, height, stride, data.len(), format.bytes_per_pixel())?;
        let src_row = width as usize * format.bytes_per_pixel();
        let dst_row = width as usize * BPP;
        let mut out = vec![0u8; dst_row * height as usize];
        for (y, dst) in out.chunks_exact_mut(dst_row).enumerate() {
            let src_y = match order {
                RowOrder::TopDown => y,
                RowOrder::BottomUp => height as usize - 1 - y,
            };
            let start = src_y * stride;
            format.convert_row(&data[start..start + src_row], dst);
        }
        Ok(Image::from_packed(width, height, out))
    }

    /// Converts a raw readback named by a DRM fourcc code into an RGBA8 image.
    pub fn from_readback_fourcc(
        data: &[u8],
        width: u32,
        height: u32,
        stride: usize,
        fourcc: u32,
        order: RowOrder,
    ) -> Result<Image, ImageError> {
        let format = PixelFormat::from_fourcc(fourcc)?;
        Self::from_readback(data, width, height, stride, format, order)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strided_bottom_up_xrgb() {
        // Two rows of two pixels with four padding bytes per row. Memory holds the bottom row first.
        let stride = 12;
        let mut data = vec![0xEEu8; stride * 2];
        data[0..8].copy_from_slice(&[1, 2, 3, 0, 4, 5, 6, 0]);
        data[12..20].copy_from_slice(&[7, 8, 9, 0, 10, 11, 12, 0]);
        let img = Image::from_readback(
            &data,
            2,
            2,
            stride,
            PixelFormat::Xrgb8888,
            RowOrder::BottomUp,
        )
        .unwrap();
        assert_eq!(img.pixel(0, 0), Some([9, 8, 7, 255]));
        assert_eq!(img.pixel(1, 0), Some([12, 11, 10, 255]));
        assert_eq!(img.pixel(0, 1), Some([3, 2, 1, 255]));
        assert_eq!(img.pixel(1, 1), Some([6, 5, 4, 255]));
        assert_eq!(img.stride(), 8);
    }

    #[test]
    fn last_row_needs_no_padding() {
        let data = vec![0u8; 12 + 8];
        assert!(
            Image::from_readback(&data, 2, 2, 12, PixelFormat::Abgr8888, RowOrder::TopDown).is_ok()
        );
        assert!(matches!(
            Image::from_readback(
                &data[..19],
                2,
                2,
                12,
                PixelFormat::Abgr8888,
                RowOrder::TopDown
            ),
            Err(ImageError::BufferTooSmall {
                needed: 20,
                got: 19
            })
        ));
    }

    #[test]
    fn fourcc_entry_point() {
        let data = (0x3ffu32 << 20).to_le_bytes();
        let img = Image::from_readback_fourcc(
            &data,
            1,
            1,
            4,
            PixelFormat::Xrgb2101010.fourcc(),
            RowOrder::TopDown,
        )
        .unwrap();
        assert_eq!(img.pixel(0, 0), Some([255, 0, 0, 255]));
    }

    /// Encodes one RGBA pixel in a format, the inverse of the conversion for opaque pixels.
    fn encode(format: PixelFormat, [r, g, b, a]: [u8; 4]) -> [u8; 4] {
        let ten = |v: u8| (v as u32) << 2 | (v as u32) >> 6;
        let a2 = (a as u32) >> 6;
        match format {
            PixelFormat::Argb8888 | PixelFormat::Xrgb8888 => [b, g, r, a],
            PixelFormat::Abgr8888 | PixelFormat::Xbgr8888 => [r, g, b, a],
            PixelFormat::Argb2101010 | PixelFormat::Xrgb2101010 => {
                (a2 << 30 | ten(r) << 20 | ten(g) << 10 | ten(b)).to_le_bytes()
            }
            PixelFormat::Abgr2101010 | PixelFormat::Xbgr2101010 => {
                (a2 << 30 | ten(b) << 20 | ten(g) << 10 | ten(r)).to_le_bytes()
            }
        }
    }

    #[test]
    fn every_format_round_trips() {
        let mut src = Image::new(7, 5);
        for y in 0..5 {
            for x in 0..7 {
                src.set_pixel(x, y, [x as u8 * 37, y as u8 * 51, (x * y) as u8 * 9, 255]);
            }
        }
        for format in PixelFormat::ALL {
            let stride = 7 * 4 + 12;
            let mut data = vec![0u8; stride * 5];
            for y in 0..5u32 {
                for x in 0..7u32 {
                    let i = (4 - y as usize) * stride + x as usize * 4;
                    let px = src.pixel(x, y).unwrap_or_default();
                    data[i..i + 4].copy_from_slice(&encode(format, px));
                }
            }
            let img = Image::from_readback(&data, 7, 5, stride, format, RowOrder::BottomUp)
                .unwrap_or_else(|e| panic!("{format:?}: {e}"));
            assert_eq!(img, src, "{format:?}");
        }
    }
}
