//! Rendering: the GLES renderer on the render node or Pixman, buffer import, dmabuf feedback,
//! capture render elements, reusable offscreen targets, chunked offscreen readback and region
//! readback of single client buffers.

mod backend;
mod buffer;
mod cursor;
mod device;
mod elements;
mod error;
mod readback;
mod targets;

pub use backend::RenderBackend;
pub use buffer::{ShmReader, read_buffer};
pub use cursor::default_cursor;
pub use device::{GlesDevice, find_render_node};
pub use elements::{CursorScene, WindowScene, collect};
pub use error::RenderError;
pub use readback::{Readback, copy_rows, create_target, into_image, map, render};
pub use targets::CaptureTargets;
