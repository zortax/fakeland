//! The owned RGBA8 image.

use fakeland_types::{Physical, Rect, Size};

use super::view::{BPP, validate};
use super::{ImageError, ImageView};

/// An owned RGBA8 image. Rows start every `stride` bytes; each row holds `width` pixels of four
/// bytes in the order red, green, blue, alpha.
#[derive(Clone, PartialEq, Eq)]
pub struct Image {
    width: u32,
    height: u32,
    stride: usize,
    data: Vec<u8>,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("stride", &self.stride)
            .finish_non_exhaustive()
    }
}

impl Image {
    /// Builds an image with every pixel set to one RGBA value. A zero dimension becomes one.
    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        let count = width as usize * height as usize;
        let mut data = Vec::with_capacity(count * BPP);
        for _ in 0..count {
            data.extend_from_slice(&rgba);
        }
        Self::from_packed(width, height, data)
    }

    /// Builds a transparent black image. A zero dimension becomes one.
    pub fn new(width: u32, height: u32) -> Self {
        let (width, height) = (width.max(1), height.max(1));
        Self::from_packed(
            width,
            height,
            vec![0; width as usize * height as usize * BPP],
        )
    }

    /// Builds an image from packed RGBA bytes, `width * 4` bytes per row.
    pub fn from_rgba(width: u32, height: u32, data: Vec<u8>) -> Result<Self, ImageError> {
        Self::from_rgba_with_stride(width, height, width as usize * BPP, data)
    }

    /// Builds an image from RGBA bytes with a row stride in bytes.
    pub fn from_rgba_with_stride(
        width: u32,
        height: u32,
        stride: usize,
        data: Vec<u8>,
    ) -> Result<Self, ImageError> {
        validate(width, height, stride, data.len(), BPP)?;
        Ok(Self {
            width,
            height,
            stride,
            data,
        })
    }

    /// Builds an image from packed bytes of the right length.
    pub(crate) fn from_packed(width: u32, height: u32, data: Vec<u8>) -> Self {
        debug_assert_eq!(data.len(), width as usize * height as usize * BPP);
        Self {
            width,
            height,
            stride: width as usize * BPP,
            data,
        }
    }

    /// Returns a borrowed view of the whole image.
    pub fn view(&self) -> ImageView<'_> {
        ImageView::from_parts(self.width, self.height, self.stride, &self.data)
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
        self.view().size()
    }

    /// Returns the underlying bytes, rows separated by the stride.
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }

    /// Returns the RGBA value at a pixel, or `None` outside the image.
    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        self.view().pixel(x, y)
    }

    /// Sets the RGBA value at a pixel. A pixel outside the image is ignored.
    pub fn set_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if x < self.width && y < self.height {
            let i = y as usize * self.stride + x as usize * BPP;
            self.data[i..i + BPP].copy_from_slice(&rgba);
        }
    }

    /// Returns the mutable pixel bytes of one row. The row must exist.
    pub fn row_mut(&mut self, y: u32) -> &mut [u8] {
        let start = y as usize * self.stride;
        &mut self.data[start..start + self.width as usize * BPP]
    }

    /// Sets every pixel of a region to one RGBA value. The region is clipped to the image.
    pub fn fill_rect(&mut self, rect: Rect<Physical>, rgba: [u8; 4]) {
        let Some(r) = self.view().clip(Some(rect)) else {
            return;
        };
        for y in r.y..r.y + r.h {
            let row = self.row_mut(y as u32);
            for px in row[r.x as usize * BPP..(r.x + r.w) as usize * BPP].chunks_exact_mut(BPP) {
                px.copy_from_slice(&rgba);
            }
        }
    }

    /// Copies a region into a new image. The region is clipped to the image. The result is `None`
    /// when the region and the image do not overlap.
    pub fn crop(&self, rect: Rect<Physical>) -> Option<Image> {
        self.view().crop(rect)
    }

    /// Copies the pixels into packed RGBA bytes, `width * 4` bytes per row.
    pub fn to_rgba_bytes(&self) -> Vec<u8> {
        self.view().to_rgba_bytes()
    }

    /// Returns packed RGBA bytes, `width * 4` bytes per row, and consumes the image.
    pub fn into_rgba_bytes(self) -> Vec<u8> {
        if self.stride == self.width as usize * BPP {
            let mut data = self.data;
            data.truncate(self.width as usize * self.height as usize * BPP);
            return data;
        }
        self.to_rgba_bytes()
    }
}

impl<'a> From<&'a Image> for ImageView<'a> {
    fn from(image: &'a Image) -> Self {
        image.view()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_and_crop() {
        let mut img = Image::filled(10, 8, [1, 2, 3, 255]);
        img.fill_rect(Rect::new(8, 6, 5, 5), [9, 9, 9, 255]);
        assert_eq!(img.pixel(7, 5), Some([1, 2, 3, 255]));
        assert_eq!(img.pixel(9, 7), Some([9, 9, 9, 255]));
        let c = img.crop(Rect::new(7, 5, 10, 10)).unwrap();
        assert_eq!((c.width(), c.height()), (3, 3));
        assert_eq!(c.pixel(0, 0), Some([1, 2, 3, 255]));
        assert_eq!(c.pixel(2, 2), Some([9, 9, 9, 255]));
        assert!(img.crop(Rect::new(20, 20, 2, 2)).is_none());
    }

    #[test]
    fn strided_bytes_pack() {
        let mut data = vec![0u8; 2 * 12];
        data[..8].copy_from_slice(&[1, 1, 1, 1, 2, 2, 2, 2]);
        data[12..20].copy_from_slice(&[3, 3, 3, 3, 4, 4, 4, 4]);
        let img = Image::from_rgba_with_stride(2, 2, 12, data).unwrap();
        assert_eq!(
            img.clone().into_rgba_bytes(),
            vec![1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4]
        );
        assert_eq!(img.pixel(1, 1), Some([4, 4, 4, 4]));
    }
}
