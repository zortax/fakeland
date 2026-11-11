//! One viewer window: a toplevel whose black background surface fills the window and whose
//! content subsurface shows the output picture letterboxed. Configures resize both surfaces with
//! viewports, and each picture waits for the frame callback of the one before.

use std::collections::HashMap;

use fakeland_types::OutputId;
use smithay::reexports::wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::client::zxdg_toplevel_decoration_v1::{
    Mode, ZxdgToplevelDecorationV1,
};
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_surface::XdgSurface;
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use wayland_client::Proxy;
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_subsurface::WlSubsurface;
use wayland_client::protocol::wl_surface::WlSurface;

use super::app::{App, BufferRef};
use super::shm::ShmBuffer;
use crate::viewer::layout::{ContentRect, fit, initial_size};
use crate::viewer::{FromViewer, OutputView, ViewerFrame};

/// The app id of every viewer window.
pub const APP_ID: &str = "fakeland-viewer";

/// The host buffer of a swapchain slot.
pub struct SlotBuffer {
    /// The buffer, once the host created it.
    pub buffer: Option<WlBuffer>,
    /// The host reads the buffer.
    pub busy: bool,
    /// The compositor dropped the slot; the buffer goes once the host releases it.
    pub retired: bool,
}

/// One window.
pub struct Window {
    /// The logical size of the output.
    pub logical: (i32, i32),
    base: WlSurface,
    content: WlSurface,
    sub: WlSubsurface,
    xdg: XdgSurface,
    toplevel: XdgToplevel,
    deco: Option<ZxdgToplevelDecorationV1>,
    base_viewport: WpViewport,
    content_viewport: WpViewport,
    background: Option<ShmBuffer>,
    /// The size from the last toplevel configure.
    pub pending_size: (i32, i32),
    /// The window size.
    pub size: (i32, i32),
    /// The place of the picture in the window.
    pub content_rect: ContentRect,
    /// The window has its first configure.
    pub configured: bool,
    /// A frame callback is outstanding.
    pub waiting: bool,
    /// Host buffers of swapchain slots.
    pub slots: HashMap<u64, SlotBuffer>,
    /// A picture whose host buffer is on the way.
    pub queued: Option<ViewerFrame>,
    /// Shared memory buffers for CPU pictures.
    pub shm: Vec<ShmBuffer>,
    /// The host imports the dmabufs of this output.
    pub dmabuf_ok: bool,
}

impl Window {
    /// Returns true when a surface belongs to the window content.
    pub fn is_content(&self, surface: &WlSurface) -> bool {
        surface.id() == self.content.id()
    }

    /// Returns the content surface.
    pub fn content(&self) -> WlSurface {
        self.content.clone()
    }
}

impl App {
    /// Opens the window of an output.
    pub fn open_window(&mut self, view: &OutputView) {
        let qh = self.qh.clone();
        let g = &self.globals;
        let base = g.compositor.create_surface(&qh, ());
        let content = g.compositor.create_surface(&qh, ());
        let sub = g.subcompositor.get_subsurface(&content, &base, &qh, ());
        sub.set_desync();
        let xdg = g.wm_base.get_xdg_surface(&base, &qh, view.id);
        let toplevel = xdg.get_toplevel(&qh, view.id);
        toplevel.set_app_id(APP_ID.into());
        toplevel.set_title(format!("fakeland {}", view.name));
        let deco = g.decorations.as_ref().map(|d| {
            let deco = d.get_toplevel_decoration(&toplevel, &qh, ());
            deco.set_mode(Mode::ServerSide);
            deco
        });
        let base_viewport = g.viewporter.get_viewport(&base, &qh, ());
        let content_viewport = g.viewporter.get_viewport(&content, &qh, ());
        let size = initial_size(view.logical);
        toplevel.set_min_size(64, 64);
        base.commit();
        self.surfaces.insert(base.id(), view.id);
        self.surfaces.insert(content.id(), view.id);
        self.windows.insert(
            view.id,
            Window {
                logical: view.logical,
                base,
                content,
                sub,
                xdg,
                toplevel,
                deco,
                base_viewport,
                content_viewport,
                background: None,
                pending_size: (0, 0),
                size,
                content_rect: fit(view.logical, size),
                configured: false,
                waiting: false,
                slots: HashMap::new(),
                queued: None,
                shm: Vec::new(),
                dmabuf_ok: self.globals.dmabuf.is_some() && !self.formats.is_empty(),
            },
        );
    }

    /// Updates the output size of a window.
    pub fn update_window(&mut self, view: &OutputView) {
        let Some(win) = self.windows.get_mut(&view.id) else {
            return;
        };
        if win.logical != view.logical {
            win.logical = view.logical;
            if win.configured {
                self.layout(view.id);
            }
        }
    }

    /// Closes the window of an output.
    pub fn close_window(&mut self, output: OutputId) {
        let Some(win) = self.windows.remove(&output) else {
            return;
        };
        self.surfaces.remove(&win.base.id());
        self.surfaces.remove(&win.content.id());
        for slot in win.slots.into_values() {
            if let Some(b) = slot.buffer {
                self.buffers.remove(&b.id());
                b.destroy();
            }
        }
        for s in win.shm {
            self.buffers.remove(&s.buffer.id());
            s.buffer.destroy();
        }
        if let Some(bg) = win.background {
            self.buffers.remove(&bg.buffer.id());
            bg.buffer.destroy();
        }
        if let Some(d) = win.deco {
            d.destroy();
        }
        win.content_viewport.destroy();
        win.base_viewport.destroy();
        win.toplevel.destroy();
        win.xdg.destroy();
        win.sub.destroy();
        win.content.destroy();
        win.base.destroy();
    }

    /// Sets the title of a window.
    pub fn set_title(&mut self, output: OutputId, title: String) {
        if let Some(win) = self.windows.get(&output) {
            win.toplevel.set_title(title);
        }
    }

    /// Applies a configure: acks it, sizes both surfaces and asks for the first picture.
    pub fn configure(&mut self, output: OutputId, serial: u32) {
        let Some(win) = self.windows.get_mut(&output) else {
            return;
        };
        win.xdg.ack_configure(serial);
        if win.pending_size.0 > 0 && win.pending_size.1 > 0 {
            win.size = win.pending_size;
        }
        if win.background.is_none() {
            let id = self.next_shm + 1;
            self.next_shm = id;
            match ShmBuffer::new(id, &self.globals.shm, 1, 1, &self.qh) {
                Ok(mut bg) => {
                    let _ = bg.write_rgba(std::iter::once(&[0u8, 0, 0, 255][..]));
                    self.buffers.insert(bg.buffer.id(), BufferRef::Background);
                    win.base.attach(Some(&bg.buffer), 0, 0);
                    win.base.damage_buffer(0, 0, 1, 1);
                    win.background = Some(bg);
                }
                Err(e) => tracing::warn!("viewer background: {e}"),
            }
        }
        let first = !win.configured;
        win.configured = true;
        self.layout(output);
        if first && let Some(win) = self.windows.get(&output) {
            let r = win.content_rect;
            self.send(FromViewer::Ready {
                output,
                width: r.w as u32,
                height: r.h as u32,
            });
        }
    }

    fn layout(&mut self, output: OutputId) {
        let Some(win) = self.windows.get_mut(&output) else {
            return;
        };
        let (w, h) = win.size;
        win.xdg.set_window_geometry(0, 0, w, h);
        win.base_viewport.set_destination(w, h);
        let r = fit(win.logical, win.size);
        win.content_rect = r;
        win.sub.set_position(r.x, r.y);
        win.content_viewport.set_destination(r.w, r.h);
        win.content.commit();
        win.base.commit();
    }
}
