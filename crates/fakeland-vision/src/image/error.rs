//! Errors of image construction, conversion and encoding.

use thiserror::Error;

/// An error from building, converting, encoding or decoding an image.
#[derive(Debug, Error)]
pub enum ImageError {
    /// The width or the height is zero.
    #[error("image size {width}x{height} has no pixels")]
    Empty {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// The row stride is smaller than one row of pixels.
    #[error("stride {stride} is smaller than the row size {row_bytes}")]
    StrideTooSmall {
        /// Bytes per row in the source.
        stride: usize,
        /// Bytes that one row of pixels needs.
        row_bytes: usize,
    },
    /// The source buffer is smaller than the image needs.
    #[error("buffer holds {got} bytes, the image needs {needed}")]
    BufferTooSmall {
        /// Bytes the image needs.
        needed: usize,
        /// Bytes the buffer holds.
        got: usize,
    },
    /// The DRM fourcc code names no supported format.
    #[error("unsupported DRM fourcc {0:#010x}")]
    UnsupportedFourcc(u32),
    /// The PNG encoder failed.
    #[error("PNG encoding failed: {0}")]
    PngEncode(#[from] png::EncodingError),
    /// The PNG decoder failed.
    #[error("PNG decoding failed: {0}")]
    PngDecode(#[from] png::DecodingError),
    /// The PNG holds a color type or bit depth this crate cannot convert.
    #[error("unsupported PNG layout {0}")]
    UnsupportedPng(String),
}
