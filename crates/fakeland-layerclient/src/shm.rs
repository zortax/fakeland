//! Solid-color wl_shm buffers: one ARGB8888 buffer over its own memfd per draw.

use std::fs::File;
use std::io::{self, Write};
use std::os::fd::AsFd;

use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::QueueHandle;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;

use crate::app::App;
use crate::args::ColorArg;

/// A buffer failure.
#[derive(Debug, thiserror::Error)]
pub enum ShmError {
    /// The memfd could not be created or written.
    #[error("shm buffer i/o: {0}")]
    Io(#[from] io::Error),
    /// The buffer is empty or too large.
    #[error("bad buffer size {0}x{1}")]
    Size(u32, u32),
}

/// Returns the premultiplied ARGB8888 bytes of a color in memory order.
pub fn pixel(color: ColorArg) -> [u8; 4] {
    let [r, g, b, a] = color.0;
    let pre = |c: u8| ((c as u32 * a as u32 + 127) / 255) as u8;
    [pre(b), pre(g), pre(r), a]
}

/// Creates a buffer of a size filled with a color. The pool goes away with the buffer.
pub fn solid_buffer(
    shm: &WlShm,
    width: u32,
    height: u32,
    color: ColorArg,
    qh: &QueueHandle<App>,
) -> Result<WlBuffer, ShmError> {
    let stride = width.checked_mul(4).ok_or(ShmError::Size(width, height))?;
    let len = stride
        .checked_mul(height)
        .filter(|l| *l > 0 && *l <= i32::MAX as u32)
        .ok_or(ShmError::Size(width, height))?;
    let fd = memfd_create("fakeland-layerclient", MemfdFlags::CLOEXEC).map_err(io::Error::from)?;
    let mut file = File::from(fd);
    let px = pixel(color);
    let row: Vec<u8> = px.iter().copied().cycle().take(stride as usize).collect();
    for _ in 0..height {
        file.write_all(&row)?;
    }
    let pool: WlShmPool = shm.create_pool(file.as_fd(), len as i32, qh, ());
    let buffer = pool.create_buffer(
        0,
        width as i32,
        height as i32,
        stride as i32,
        wl_shm::Format::Argb8888,
        qh,
        (),
    );
    pool.destroy();
    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixels_are_premultiplied_bgra() {
        assert_eq!(pixel(ColorArg([255, 0, 0, 255])), [0, 0, 255, 255]);
        assert_eq!(pixel(ColorArg([255, 255, 255, 0])), [0, 0, 0, 0]);
        assert_eq!(pixel(ColorArg([200, 100, 50, 128])), [25, 50, 100, 128]);
    }
}
