//! A borrowed RGBA8 image with a row stride. A view can name a region of a larger image without a
//! copy.

use fakeland_types::{Physical, Rect, Size};

use super::{Image, ImageError};

/// Bytes per RGBA8 pixel.
pub(crate) const BPP: usize = 4;

/// A borrowed RGBA8 image. Rows start every `stride` bytes; each row holds `width` pixels of four
/// bytes in the order red, green, blue, alpha.
#[derive(Clone, Copy, Debug)]
pub struct ImageView<'a> {
    width: u32,
    height: u32,
    stride: usize,
    data: &'a [u8],
}

/// Returns the byte count that an image with these dimensions needs.
pub(crate) fn required_len(width: u32, height: u32, stride: usize) -> usize {
    if height == 0 {
        return 0;
    }
    (height as usize - 1) * stride + width as usize * BPP
}

/// Checks the dimensions, the stride and the buffer length of an RGBA8 image.
pub(crate) fn validate(
    width: u32,
    height: u32,
    stride: usize,
    len: usize,
    bpp: usize,
) -> Result<(), ImageError> {
    if width == 0 || height == 0 {
        return Err(ImageError::Empty { width, height });
    }
    let row_bytes = width as usize * bpp;
    if stride < row_bytes {
        return Err(ImageError::StrideTooSmall { stride, row_bytes });
    }
    let needed = (height as usize - 1) * stride + row_bytes;
    if len < needed {
        return Err(ImageError::BufferTooSmall { needed, got: len });
    }
    Ok(())
}

impl<'a> ImageView<'a> {
    /// Builds a view over RGBA8 bytes with a row stride in bytes.
    pub fn new(width: u32, height: u32, stride: usize, data: &'a [u8]) -> Result<Self, ImageError> {
        validate(width, height, stride, data.len(), BPP)?;
        Ok(Self {
            width,
            height,
            stride,
            data: &data[..required_len(width, height, stride)],
        })
    }

    /// Builds a view without checks. The caller guarantees a valid layout.
    pub(crate) fn from_parts(width: u32, height: u32, stride: usize, data: &'a [u8]) -> Self {
        debug_assert!(validate(width, height, stride, data.len(), BPP).is_ok());
        Self {
            width,
            height,
            stride,
            data,
        }
    }

    /// Returns the width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns the bytes between the starts of two rows.
    pub fn stride(&self) -> usize {
        self.stride
    }

    /// Returns the size.
    pub fn size(&self) -> Size<Physical> {
        Size::new(self.width as i32, self.height as i32)
    }

    /// Returns the rectangle that covers the whole image.
    pub fn bounds(&self) -> Rect<Physical> {
        Rect::new(0, 0, self.width as i32, self.height as i32)
    }

    /// Returns the underlying bytes, rows separated by the stride.
    pub fn as_bytes(&self) -> &'a [u8] {
        self.data
    }

    /// Returns the pixel bytes of one row. The row must exist.
    pub fn row(&self, y: u32) -> &'a [u8] {
        let start = y as usize * self.stride;
        &self.data[start..start + self.width as usize * BPP]
    }

    /// Returns an iterator over the pixel bytes of each row, top to bottom.
    pub fn rows(&self) -> impl Iterator<Item = &'a [u8]> + 'a {
        let view = *self;
        (0..view.height).map(move |y| view.row(y))
    }

    /// Returns the RGBA value at a pixel, or `None` outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = y as usize * self.stride + x as usize * BPP;
        Some([
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ])
    }

    /// Returns the part of a region inside the image. `None` selects the whole image. The result is
    /// `None` when the region and the image do not overlap.
    pub fn clip(&self, roi: Option<Rect<Physical>>) -> Option<Rect<Physical>> {
        match roi {
            None => Some(self.bounds()),
            Some(r) => r.intersection(self.bounds()),
        }
    }

    /// Returns a view of a region, clipped to the image. The result is `None` when the region and
    /// the image do not overlap.
    pub fn region(&self, rect: Rect<Physical>) -> Option<ImageView<'a>> {
        let r = self.clip(Some(rect))?;
        let start = r.y as usize * self.stride + r.x as usize * BPP;
        let len = required_len(r.w as u32, r.h as u32, self.stride);
        Some(Self::from_parts(
            r.w as u32,
            r.h as u32,
            self.stride,
            &self.data[start..start + len],
        ))
    }

    /// Returns true when the two views have the same width and height.
    pub fn same_size(&self, other: &ImageView<'_>) -> bool {
        self.width == other.width && self.height == other.height
    }

    /// Copies the view into an owned image with packed rows.
    pub fn to_image(&self) -> Image {
        Image::from_packed(self.width, self.height, self.to_rgba_bytes())
    }

    /// Copies a region into an owned image. The region is clipped to the image. The result is
    /// `None` when the region and the image do not overlap.
    pub fn crop(&self, rect: Rect<Physical>) -> Option<Image> {
        self.region(rect).map(|v| v.to_image())
    }

    /// Copies the pixels into packed RGBA bytes, `width * 4` bytes per row.
    pub fn to_rgba_bytes(&self) -> Vec<u8> {
        let row_bytes = self.width as usize * BPP;
        if self.stride == row_bytes {
            return self.data[..row_bytes * self.height as usize].to_vec();
        }
        let mut out = Vec::with_capacity(row_bytes * self.height as usize);
        for row in self.rows() {
            out.extend_from_slice(row);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered(w: u32, h: u32, stride: usize) -> Vec<u8> {
        let mut data = vec![0u8; required_len(w, h, stride)];
        for y in 0..h as usize {
            for x in 0..w as usize {
                let i = y * stride + x * BPP;
                data[i] = x as u8;
                data[i + 1] = y as u8;
                data[i + 2] = 7;
                data[i + 3] = 255;
            }
        }
        data
    }

    #[test]
    fn rejects_bad_layouts() {
        let data = vec![0u8; 64];
        assert!(matches!(
            ImageView::new(0, 4, 16, &data),
            Err(ImageError::Empty { .. })
        ));
        assert!(matches!(
            ImageView::new(4, 4, 8, &data),
            Err(ImageError::StrideTooSmall { .. })
        ));
        assert!(matches!(
            ImageView::new(4, 5, 16, &data),
            Err(ImageError::BufferTooSmall { .. })
        ));
        assert!(ImageView::new(4, 4, 16, &data).is_ok());
    }

    #[test]
    fn region_reads_through_the_stride() {
        let data = numbered(8, 6, 40);
        let view = ImageView::new(8, 6, 40, &data).unwrap();
        let sub = view.region(Rect::new(2, 3, 4, 2)).unwrap();
        assert_eq!((sub.width(), sub.height()), (4, 2));
        assert_eq!(sub.pixel(0, 0), Some([2, 3, 7, 255]));
        assert_eq!(sub.pixel(3, 1), Some([5, 4, 7, 255]));
        assert_eq!(sub.pixel(4, 0), None);
        let img = sub.to_image();
        assert_eq!(img.stride(), 16);
        assert_eq!(img.view().pixel(3, 1), Some([5, 4, 7, 255]));
    }

    #[test]
    fn region_clips_and_rejects_outside() {
        let data = numbered(8, 6, 32);
        let view = ImageView::new(8, 6, 32, &data).unwrap();
        let sub = view.region(Rect::new(-2, 4, 5, 10)).unwrap();
        assert_eq!((sub.width(), sub.height()), (3, 2));
        assert_eq!(sub.pixel(0, 0), Some([0, 4, 7, 255]));
        assert!(view.region(Rect::new(8, 0, 2, 2)).is_none());
        assert_eq!(view.clip(None), Some(Rect::new(0, 0, 8, 6)));
    }
}
