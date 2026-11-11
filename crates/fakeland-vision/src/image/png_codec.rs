//! PNG encoding and decoding of RGBA8 images.

use std::io::Cursor;

use png::{BitDepth, ColorType, Compression, Transformations};

use super::view::BPP;
use super::{Image, ImageError, ImageView};

impl ImageView<'_> {
    /// Encodes the view as an 8-bit RGBA PNG.
    pub fn encode_png(&self) -> Result<Vec<u8>, ImageError> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, self.width(), self.height());
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_compression(Compression::Fast);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(&self.to_rgba_bytes())?;
        writer.finish()?;
        Ok(out)
    }
}

impl Image {
    /// Encodes the image as an 8-bit RGBA PNG.
    pub fn encode_png(&self) -> Result<Vec<u8>, ImageError> {
        self.view().encode_png()
    }

    /// Decodes a PNG into RGBA8. Palette, gray and 16-bit images are expanded to RGBA8.
    pub fn decode_png(bytes: &[u8]) -> Result<Image, ImageError> {
        let mut decoder = png::Decoder::new(Cursor::new(bytes));
        decoder.set_transformations(
            Transformations::EXPAND | Transformations::STRIP_16 | Transformations::ALPHA,
        );
        let mut reader = decoder.read_info()?;
        let size = reader
            .output_buffer_size()
            .ok_or_else(|| ImageError::UnsupportedPng("frame too large".into()))?;
        let mut buf = vec![0u8; size];
        let info = reader.next_frame(&mut buf)?;
        let (width, height) = (info.width, info.height);
        if info.bit_depth != BitDepth::Eight {
            return Err(ImageError::UnsupportedPng(format!(
                "bit depth {:?}",
                info.bit_depth
            )));
        }
        let count = width as usize * height as usize;
        let data = match info.color_type {
            ColorType::Rgba => {
                buf.truncate(count * BPP);
                buf
            }
            ColorType::GrayscaleAlpha => {
                let mut out = Vec::with_capacity(count * BPP);
                for px in buf.chunks_exact(2).take(count) {
                    out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
                }
                out
            }
            other => return Err(ImageError::UnsupportedPng(format!("color type {other:?}"))),
        };
        Image::from_rgba(width, height, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_keeps_pixels() {
        let mut img = Image::new(37, 11);
        for y in 0..11 {
            for x in 0..37 {
                img.set_pixel(
                    x,
                    y,
                    [x as u8 * 7, y as u8 * 23, (x * y) as u8, 255 - x as u8],
                );
            }
        }
        let bytes = img.encode_png().unwrap();
        assert_eq!(&bytes[1..4], b"PNG");
        let back = Image::decode_png(&bytes).unwrap();
        assert_eq!(back, img);
    }

    #[test]
    fn gray_png_expands() {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, 2, 1);
            enc.set_color(ColorType::Grayscale);
            enc.set_depth(BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[10, 200]).unwrap();
        }
        let img = Image::decode_png(&out).unwrap();
        assert_eq!(img.pixel(0, 0), Some([10, 10, 10, 255]));
        assert_eq!(img.pixel(1, 0), Some([200, 200, 200, 255]));
    }

    #[test]
    fn garbage_fails() {
        assert!(matches!(
            Image::decode_png(b"not a png"),
            Err(ImageError::PngDecode(_))
        ));
    }
}
