//! The wl_shm buffer pool. Each slot is one XRGB8888 wl_buffer over its own memfd. A slot keeps
//! a shadow copy of its memfd content, so an upload writes only the band of rows that differ.
//! Slots are allocated on demand up to a maximum; a free slot of the wrong size is reallocated.

use std::fs::File;
use std::io;
use std::os::fd::AsFd;
use std::os::unix::fs::FileExt;

use fakeland_types::{Physical, Size};
use rustix::fs::{MemfdFlags, memfd_create};
use wayland_client::QueueHandle;
use wayland_client::protocol::{wl_buffer::WlBuffer, wl_shm, wl_shm::WlShm};

use crate::app::App;
use crate::draw::{BYTES_PER_PIXEL, changed_rows};

/// The user data of a wl_buffer: its buffer number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BufferTag(pub u64);

/// A buffer pool failure.
#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    /// The memfd could not be created, sized or written.
    #[error("shm buffer i/o: {0}")]
    Io(#[from] io::Error),
    /// The buffer is too large for wl_shm.
    #[error("buffer {0}x{1} is too large")]
    TooLarge(i32, i32),
}

/// One buffer.
#[derive(Debug)]
struct Slot {
    number: u64,
    file: File,
    buffer: WlBuffer,
    size: Size<Physical>,
    busy: bool,
    frame_id: u64,
    shadow: Vec<u8>,
}

/// The buffers of the surface.
#[derive(Debug)]
pub struct BufferPool {
    shm: WlShm,
    slots: Vec<Slot>,
    max: usize,
    next_number: u64,
}

impl BufferPool {
    /// Builds an empty pool of at most `max` buffers.
    pub fn new(shm: WlShm, max: usize) -> Self {
        Self {
            shm,
            slots: Vec::new(),
            max: max.max(1),
            next_number: 1,
        }
    }

    /// Returns true when `acquire` can return a buffer.
    pub fn can_acquire(&self) -> bool {
        self.slots.len() < self.max || self.slots.iter().any(|s| !s.busy)
    }

    /// Returns the index of a free buffer of a size, allocating or reallocating one when needed.
    /// Returns `None` while every buffer is busy and the pool is full.
    pub fn acquire(
        &mut self,
        size: Size<Physical>,
        qh: &QueueHandle<App>,
    ) -> Result<Option<usize>, PoolError> {
        if let Some(i) = self.slots.iter().position(|s| !s.busy && s.size == size) {
            return Ok(Some(i));
        }
        if let Some(i) = self.slots.iter().position(|s| !s.busy) {
            let slot = self.allocate(size, qh)?;
            let old = std::mem::replace(&mut self.slots[i], slot);
            old.buffer.destroy();
            return Ok(Some(i));
        }
        if self.slots.len() < self.max {
            let slot = self.allocate(size, qh)?;
            self.slots.push(slot);
            return Ok(Some(self.slots.len() - 1));
        }
        Ok(None)
    }

    /// Returns the wl_buffer and the buffer number of a slot.
    pub fn buffer(&self, index: usize) -> (&WlBuffer, u64) {
        let s = &self.slots[index];
        (&s.buffer, s.number)
    }

    /// Writes a frame into a slot. Only the rows that differ from the slot's content are
    /// written.
    pub fn upload(&mut self, index: usize, frame: &[u8]) -> Result<(), PoolError> {
        let slot = &mut self.slots[index];
        let stride = slot.size.w as usize * BYTES_PER_PIXEL;
        let Some((y0, y1)) = changed_rows(&slot.shadow, frame, slot.size) else {
            return Ok(());
        };
        let (a, b) = (y0 * stride, y1 * stride);
        slot.file.write_all_at(&frame[a..b], a as u64)?;
        slot.shadow[a..b].copy_from_slice(&frame[a..b]);
        Ok(())
    }

    /// Marks a slot as held by the compositor for a frame.
    pub fn attach(&mut self, index: usize, frame_id: u64) {
        let slot = &mut self.slots[index];
        slot.busy = true;
        slot.frame_id = frame_id;
    }

    /// Records wl_buffer.release. Returns the frame that the buffer showed.
    pub fn release(&mut self, tag: BufferTag) -> Option<u64> {
        let slot = self.slots.iter_mut().find(|s| s.number == tag.0)?;
        slot.busy = false;
        Some(slot.frame_id)
    }

    fn allocate(&mut self, size: Size<Physical>, qh: &QueueHandle<App>) -> Result<Slot, PoolError> {
        let stride = size.w as i64 * BYTES_PER_PIXEL as i64;
        let len = stride * size.h as i64;
        if size.w <= 0 || size.h <= 0 || len > i32::MAX as i64 {
            return Err(PoolError::TooLarge(size.w, size.h));
        }
        let fd =
            memfd_create("fakeland-testclient", MemfdFlags::CLOEXEC).map_err(io::Error::from)?;
        let file = File::from(fd);
        file.set_len(len as u64)?;
        let number = self.next_number;
        self.next_number += 1;
        let pool = self.shm.create_pool(file.as_fd(), len as i32, qh, ());
        let buffer = pool.create_buffer(
            0,
            size.w,
            size.h,
            stride as i32,
            wl_shm::Format::Xrgb8888,
            qh,
            BufferTag(number),
        );
        pool.destroy();
        Ok(Slot {
            number,
            file,
            buffer,
            size,
            busy: false,
            frame_id: 0,
            shadow: vec![0; len as usize],
        })
    }
}
