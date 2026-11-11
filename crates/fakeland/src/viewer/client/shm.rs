//! Shared memory buffers of the viewer: memfd files written with `pwrite`, so no mapping is
//! needed. Pixels go from RGBA to the little-endian XRGB8888 byte order.

use std::os::fd::{AsFd, OwnedFd};

use smithay::reexports::rustix::fs::{MemfdFlags, ftruncate, memfd_create};
use smithay::reexports::rustix::io::pwrite;
use wayland_client::QueueHandle;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_shm::{Format, WlShm};

use super::app::App;

/// One shared memory buffer.
pub struct ShmBuffer {
    /// A unique id.
    pub id: u64,
    /// The host buffer.
    pub buffer: WlBuffer,
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
    /// The host reads the buffer.
    pub busy: bool,
    fd: OwnedFd,
    scratch: Vec<u8>,
}

impl ShmBuffer {
    /// Allocates a buffer.
    pub fn new(
        id: u64,
        shm: &WlShm,
        width: u32,
        height: u32,
        qh: &QueueHandle<App>,
    ) -> std::io::Result<Self> {
        let size = width as u64 * height as u64 * 4;
        let fd = memfd_create("fakeland-viewer", MemfdFlags::CLOEXEC)?;
        ftruncate(&fd, size)?;
        let pool = shm.create_pool(fd.as_fd(), size as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            width as i32 * 4,
            Format::Xrgb8888,
            qh,
            (),
        );
        pool.destroy();
        Ok(Self {
            id,
            buffer,
            width,
            height,
            busy: false,
            fd,
            scratch: Vec::new(),
        })
    }

    /// Writes RGBA rows into the buffer.
    pub fn write_rgba<'a>(&mut self, rows: impl Iterator<Item = &'a [u8]>) -> std::io::Result<()> {
        self.scratch.clear();
        for row in rows {
            rgba_to_xrgb(&row[..self.width as usize * 4], &mut self.scratch);
        }
        let mut done = 0;
        while done < self.scratch.len() {
            done += pwrite(&self.fd, &self.scratch[done..], done as u64)?;
        }
        Ok(())
    }
}

/// Appends RGBA pixels as XRGB8888 in memory order (B, G, R, X).
pub fn rgba_to_xrgb(rgba: &[u8], out: &mut Vec<u8>) {
    out.reserve(rgba.len());
    for px in rgba.chunks_exact(4) {
        out.extend_from_slice(&[px[2], px[1], px[0], 255]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn swaps_red_and_blue() {
        let mut out = Vec::new();
        rgba_to_xrgb(&[1, 2, 3, 4, 10, 20, 30, 40], &mut out);
        assert_eq!(out, vec![3, 2, 1, 255, 30, 20, 10, 255]);
    }
}
