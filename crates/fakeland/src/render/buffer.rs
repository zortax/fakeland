//! Readback of a region of one client buffer at buffer resolution, without composition. A dmabuf
//! imports into the GLES renderer, an shm buffer maps into a CPU renderer, and a single-pixel
//! buffer fills the region with its color. The region renders into an offscreen target of its own
//! size, so the cost scales with the region.

use fakeland_types::{Physical, Rect};
use fakeland_vision::{Image, PixelFormat, RowOrder};
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::gles::{GlesRenderbuffer, GlesRenderer};
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::{
    Bind, Color32F, ExportMem, Frame, ImportDma, ImportMemWl, Offscreen, Renderer, Texture,
};
use smithay::reexports::pixman;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::utils::{Buffer as BufferCoords, Physical as SPhysical, Rectangle, Size, Transform};
use smithay::wayland::dmabuf::get_dmabuf;
use smithay::wayland::single_pixel_buffer::get_single_pixel_buffer;

use super::RenderBackend;

/// A CPU renderer for shm readbacks next to the GLES renderer.
#[derive(Default)]
pub struct ShmReader {
    renderer: Option<PixmanRenderer>,
}

impl std::fmt::Debug for ShmReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShmReader")
            .field("open", &self.renderer.is_some())
            .finish()
    }
}

impl ShmReader {
    fn renderer(&mut self) -> Result<&mut PixmanRenderer, String> {
        if self.renderer.is_none() {
            self.renderer = Some(PixmanRenderer::new().map_err(|e| e.to_string())?);
        }
        self.renderer
            .as_mut()
            .ok_or_else(|| "no CPU renderer".to_string())
    }
}

/// Reads a region of a client buffer, in buffer pixels. The region is clipped to the buffer.
pub fn read_buffer(
    backend: &mut RenderBackend,
    shm: &mut ShmReader,
    buffer: &WlBuffer,
    region: Rect<Physical>,
) -> Result<Image, String> {
    if let Ok(spb) = get_single_pixel_buffer(buffer) {
        let size = region.size();
        if size.w <= 0 || size.h <= 0 {
            return Err("the region is empty".into());
        }
        return Ok(Image::filled(size.w as u32, size.h as u32, spb.rgba8888()));
    }
    if let Ok(dmabuf) = get_dmabuf(buffer) {
        let dmabuf = dmabuf.clone();
        return match backend {
            RenderBackend::Gles(dev) => {
                let renderer = dev.renderer().ok_or("no GLES renderer")?;
                let texture = renderer
                    .import_dmabuf(&dmabuf, None)
                    .map_err(|e| format!("dmabuf import: {e}"))?;
                read_texture::<GlesRenderer, GlesRenderbuffer>(renderer, &texture, region)
            }
            RenderBackend::Pixman(renderer) => {
                let texture = renderer
                    .import_dmabuf(&dmabuf, None)
                    .map_err(|e| format!("dmabuf import: {e}"))?;
                read_texture::<PixmanRenderer, pixman::Image<'static, 'static>>(
                    renderer, &texture, region,
                )
            }
        };
    }
    let renderer = match backend {
        RenderBackend::Pixman(renderer) => renderer,
        RenderBackend::Gles(_) => shm.renderer()?,
    };
    let texture = renderer
        .import_shm_buffer(buffer, None, &[])
        .map_err(|e| format!("shm import: {e}"))?;
    read_texture::<PixmanRenderer, pixman::Image<'static, 'static>>(renderer, &texture, region)
}

/// Renders a region of a texture onto a target of the region size and reads it back.
fn read_texture<R, T>(
    renderer: &mut R,
    texture: &R::TextureId,
    region: Rect<Physical>,
) -> Result<Image, String>
where
    R: Renderer + Offscreen<T> + Bind<T> + ExportMem,
{
    let full = texture.size();
    let src = Rect::<Physical>::new(0, 0, full.w, full.h)
        .intersection(region)
        .ok_or_else(|| "the region lies outside the buffer".to_string())?;
    let size: Size<i32, SPhysical> = (src.w, src.h).into();
    let buffer_size: Size<i32, BufferCoords> = (src.w, src.h).into();
    let mut target: T = renderer
        .create_buffer(Fourcc::Abgr8888, buffer_size)
        .map_err(|e| format!("offscreen buffer: {e}"))?;
    let mut framebuffer = renderer
        .bind(&mut target)
        .map_err(|e| format!("bind: {e}"))?;
    let dst = Rectangle::from_size(size);
    {
        let mut frame = renderer
            .render(&mut framebuffer, size, Transform::Normal)
            .map_err(|e| format!("render: {e}"))?;
        frame
            .clear(Color32F::TRANSPARENT, &[dst])
            .map_err(|e| format!("clear: {e}"))?;
        let src_rect: Rectangle<f64, BufferCoords> =
            Rectangle::new((src.x, src.y).into(), (src.w, src.h).into()).to_f64();
        frame
            .render_texture_from_to(texture, src_rect, dst, &[dst], &[], Transform::Normal, 1.0)
            .map_err(|e| format!("draw: {e}"))?;
        let sync = frame.finish().map_err(|e| format!("finish: {e}"))?;
        renderer.wait(&sync).map_err(|e| format!("wait: {e}"))?;
    }
    let mapping = renderer
        .copy_framebuffer(
            &framebuffer,
            Rectangle::from_size(buffer_size),
            Fourcc::Abgr8888,
        )
        .map_err(|e| format!("copy: {e}"))?;
    let bytes = renderer
        .map_texture(&mapping)
        .map_err(|e| format!("map: {e}"))?;
    Image::from_readback(
        bytes,
        src.w as u32,
        src.h as u32,
        src.w as usize * 4,
        PixelFormat::Abgr8888,
        RowOrder::TopDown,
    )
    .map_err(|e| format!("readback: {e}"))
}
