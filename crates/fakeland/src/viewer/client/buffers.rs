//! Pictures on a viewer window: dmabuf slots become host buffers once through
//! `zwp_linux_buffer_params_v1`, pixel pictures go through shared memory, and each shown picture
//! asks for a frame callback before the next.

use fakeland_types::OutputId;
use smithay::backend::allocator::Buffer;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::reexports::wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_buffer_params_v1::Flags;
use wayland_client::Proxy;
use wayland_client::protocol::wl_buffer::WlBuffer;

use super::app::{App, BufferRef};
use super::shm::ShmBuffer;
use super::window::SlotBuffer;
use crate::viewer::{FromViewer, ViewerFrame};

/// The shared memory buffers per window.
const SHM_BUFFERS: usize = 3;

impl App {
    /// Shows a picture, or queues it until its host buffer exists.
    pub fn show(&mut self, output: OutputId, frame: ViewerFrame) {
        let Some(win) = self.windows.get_mut(&output) else {
            return;
        };
        match frame {
            ViewerFrame::Dmabuf { id, ref dmabuf, .. } => {
                if !win.dmabuf_ok {
                    self.send(FromViewer::DmabufFailed { output });
                    self.send_ready(output);
                    return;
                }
                let ready = win.slots.get(&id).is_some_and(|s| s.buffer.is_some());
                if !ready {
                    if !win.slots.contains_key(&id) {
                        self.create_slot(output, id, dmabuf);
                    }
                    if let Some(win) = self.windows.get_mut(&output) {
                        win.queued = Some(frame);
                    }
                    return;
                }
                self.attach_slot(output, frame);
            }
            ViewerFrame::Pixels(image) => self.attach_pixels(output, &image),
        }
    }

    fn create_slot(&mut self, output: OutputId, id: u64, dmabuf: &Dmabuf) {
        let Some(global) = &self.globals.dmabuf else {
            return;
        };
        let params = global.create_params(&self.qh, (output, id));
        let modifier: u64 = dmabuf.format().modifier.into();
        for (plane, ((fd, offset), stride)) in dmabuf
            .handles()
            .zip(dmabuf.offsets())
            .zip(dmabuf.strides())
            .enumerate()
        {
            params.add(
                fd,
                plane as u32,
                offset,
                stride,
                (modifier >> 32) as u32,
                modifier as u32,
            );
        }
        let size = dmabuf.size();
        params.create(size.w, size.h, dmabuf.format().code as u32, Flags::empty());
        if let Some(win) = self.windows.get_mut(&output) {
            win.slots.insert(
                id,
                SlotBuffer {
                    buffer: None,
                    busy: false,
                    retired: false,
                },
            );
        }
    }

    /// Stores the host buffer of a slot and shows the queued picture.
    pub fn slot_created(&mut self, output: OutputId, id: u64, buffer: WlBuffer) {
        self.buffers
            .insert(buffer.id(), BufferRef::Slot(output, id));
        let Some(win) = self.windows.get_mut(&output) else {
            buffer.destroy();
            return;
        };
        match win.slots.get_mut(&id) {
            Some(slot) => slot.buffer = Some(buffer),
            None => {
                buffer.destroy();
                return;
            }
        }
        let queued_here =
            matches!(&win.queued, Some(ViewerFrame::Dmabuf { id: q, .. }) if *q == id);
        if queued_here && let Some(frame) = win.queued.take() {
            self.attach_slot(output, frame);
        }
    }

    /// Switches a window to CPU pictures after the host refused a dmabuf.
    pub fn slot_failed(&mut self, output: OutputId, id: u64) {
        let Some(win) = self.windows.get_mut(&output) else {
            return;
        };
        tracing::warn!("the host cannot import the viewer dmabuf, using shared memory");
        win.dmabuf_ok = false;
        win.slots.remove(&id);
        win.queued = None;
        self.send(FromViewer::Released { output, id });
        self.send(FromViewer::DmabufFailed { output });
        self.send_ready(output);
    }

    fn attach_slot(&mut self, output: OutputId, frame: ViewerFrame) {
        let ViewerFrame::Dmabuf { id, sync, .. } = frame else {
            return;
        };
        let Some(win) = self.windows.get_mut(&output) else {
            return;
        };
        let Some(slot) = win.slots.get_mut(&id) else {
            return;
        };
        let Some(buffer) = slot.buffer.clone() else {
            return;
        };
        if sync.wait().is_err() {
            tracing::debug!("viewer sync wait interrupted");
        }
        slot.busy = true;
        self.commit_content(output, &buffer, i32::MAX, i32::MAX);
    }

    fn attach_pixels(&mut self, output: OutputId, image: &fakeland_vision::Image) {
        let (w, h) = (image.width(), image.height());
        let Some(win) = self.windows.get_mut(&output) else {
            return;
        };
        let mut stale = Vec::new();
        win.shm.retain(|b| {
            let keep = b.busy || (b.width == w && b.height == h);
            if !keep {
                stale.push(b.buffer.clone());
            }
            keep
        });
        let free = win
            .shm
            .iter()
            .position(|b| !b.busy && b.width == w && b.height == h);
        let count = win.shm.len();
        for b in stale {
            self.buffers.remove(&b.id());
            b.destroy();
        }
        let index = match free {
            Some(i) => i,
            None if count < SHM_BUFFERS => {
                let id = self.shm_id();
                match ShmBuffer::new(id, &self.globals.shm, w, h, &self.qh) {
                    Ok(b) => {
                        self.buffers
                            .insert(b.buffer.id(), BufferRef::Shm(output, id));
                        let Some(win) = self.windows.get_mut(&output) else {
                            return;
                        };
                        win.shm.push(b);
                        win.shm.len() - 1
                    }
                    Err(e) => {
                        tracing::warn!("viewer shm buffer: {e}");
                        return;
                    }
                }
            }
            None => {
                self.send_ready(output);
                return;
            }
        };
        let Some(win) = self.windows.get_mut(&output) else {
            return;
        };
        let buf = &mut win.shm[index];
        if let Err(e) = buf.write_rgba(image.view().rows()) {
            tracing::warn!("viewer shm write: {e}");
            return;
        }
        buf.busy = true;
        let buffer = buf.buffer.clone();
        self.commit_content(output, &buffer, w as i32, h as i32);
    }

    fn commit_content(&mut self, output: OutputId, buffer: &WlBuffer, w: i32, h: i32) {
        let qh = self.qh.clone();
        let Some(win) = self.windows.get_mut(&output) else {
            return;
        };
        let content = win.content();
        content.attach(Some(buffer), 0, 0);
        content.damage_buffer(0, 0, w, h);
        content.frame(&qh, output);
        content.commit();
        win.waiting = true;
    }

    /// Runs when the host shows the last picture: the next queued picture goes up, or the
    /// compositor gets a request for one.
    pub fn frame_done(&mut self, output: OutputId) {
        let Some(win) = self.windows.get_mut(&output) else {
            return;
        };
        win.waiting = false;
        self.send_ready(output);
    }

    /// Asks the compositor for the next picture of an output.
    pub fn send_ready(&self, output: OutputId) {
        if let Some(win) = self.windows.get(&output) {
            let r = win.content_rect;
            self.send(FromViewer::Ready {
                output,
                width: r.w as u32,
                height: r.h as u32,
            });
        }
    }

    /// Handles the release of a host buffer.
    pub fn released(&mut self, buffer: &WlBuffer) {
        let Some(owner) = self.buffers.get(&buffer.id()).copied() else {
            return;
        };
        match owner {
            BufferRef::Slot(output, id) => {
                if let Some(win) = self.windows.get_mut(&output)
                    && let Some(slot) = win.slots.get_mut(&id)
                {
                    slot.busy = false;
                    if slot.retired {
                        self.drop_slot(output, id);
                    }
                }
                self.send(FromViewer::Released { output, id });
            }
            BufferRef::Shm(output, id) => {
                if let Some(win) = self.windows.get_mut(&output)
                    && let Some(b) = win.shm.iter_mut().find(|b| b.id == id)
                {
                    b.busy = false;
                }
            }
            BufferRef::Background => {}
        }
    }

    /// Drops the host buffers of slots that the compositor no longer uses.
    pub fn retire(&mut self, output: OutputId, ids: &[u64]) {
        for id in ids {
            let busy = self
                .windows
                .get_mut(&output)
                .and_then(|w| w.slots.get_mut(id))
                .map(|s| {
                    s.retired = true;
                    s.busy
                });
            if busy == Some(false) {
                self.drop_slot(output, *id);
            }
        }
    }

    fn drop_slot(&mut self, output: OutputId, id: u64) {
        let Some(slot) = self
            .windows
            .get_mut(&output)
            .and_then(|w| w.slots.remove(&id))
        else {
            return;
        };
        if let Some(b) = slot.buffer {
            self.buffers.remove(&b.id());
            b.destroy();
        }
    }
}
