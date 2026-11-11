//! Offscreen composition and readback. The elements render into an offscreen target, and the GPU
//! copies a bounded region of it into a pack buffer. A fence signals when the copy is complete.
//! The rows then come back in chunks, so no single step blocks the compositor thread for long.
//! Smithay offscreen targets of both renderers read back with the top row first.

use std::ops::Range;

use fakeland_vision::Image;
use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::RenderElement;
use smithay::backend::renderer::sync::SyncPoint;
use smithay::backend::renderer::{Bind, ExportMem, Frame, Offscreen, Renderer, Texture};
use smithay::utils::{Physical, Rectangle, Size, Transform};

/// The background of every capture.
const CLEAR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// The bytes of one RGBA pixel.
const BPP: usize = 4;

/// A readback in flight.
pub struct Readback<M> {
    mapping: M,
    fence: SyncPoint,
    width: u32,
    height: u32,
}

impl<M> Readback<M> {
    /// Returns the width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns the height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns the fence that signals when the pixels are in the pack buffer.
    pub fn fence(&self) -> &SyncPoint {
        &self.fence
    }
}

/// Creates an offscreen target of a size.
pub fn create_target<R, T>(renderer: &mut R, size: Size<i32, Physical>) -> Result<T, String>
where
    R: Offscreen<T>,
{
    renderer
        .create_buffer(
            Fourcc::Abgr8888,
            size.to_logical(1).to_buffer(1, Transform::Normal),
        )
        .map_err(|e| format!("offscreen buffer: {e}"))
}

/// Renders `elements` onto `target` and starts the readback of `read` when given.
pub fn render<R, T, E>(
    renderer: &mut R,
    target: &mut T,
    elements: &[E],
    size: Size<i32, Physical>,
    scale: f64,
    read: Option<Rectangle<i32, Physical>>,
) -> Result<Option<Readback<R::TextureMapping>>, String>
where
    R: Renderer + Bind<T> + ExportMem,
    E: RenderElement<R>,
{
    let mut framebuffer = renderer.bind(target).map_err(|e| format!("bind: {e}"))?;
    let mut tracker = OutputDamageTracker::new(size, scale, Transform::Normal);
    tracker
        .render_output(renderer, &mut framebuffer, 0, elements, CLEAR)
        .map_err(|e| format!("render: {e:?}"))?;
    let Some(read) = read else { return Ok(None) };
    let read = read
        .intersection(Rectangle::from_size(size))
        .ok_or_else(|| "the region lies outside the target".to_string())?;
    let region = Rectangle::new(
        (read.loc.x, read.loc.y).into(),
        (read.size.w, read.size.h).into(),
    );
    let mapping = renderer
        .copy_framebuffer(&framebuffer, region, Fourcc::Abgr8888)
        .map_err(|e| format!("copy: {e}"))?;
    // An empty frame after the copy puts a fence behind it.
    let fence = renderer
        .render(&mut framebuffer, size, Transform::Normal)
        .map_err(|e| format!("fence frame: {e}"))?
        .finish()
        .map_err(|e| format!("fence: {e}"))?;
    let (width, height) = (mapping.width(), mapping.height());
    Ok(Some(Readback {
        mapping,
        fence,
        width,
        height,
    }))
}

/// Maps the pack buffer of a readback into memory. The mapping stays until the readback drops.
pub fn map<R>(renderer: &mut R, readback: &Readback<R::TextureMapping>) -> Result<(), String>
where
    R: ExportMem,
{
    renderer
        .map_texture(&readback.mapping)
        .map(|_| ())
        .map_err(|e| format!("map: {e}"))
}

/// Appends the bytes of the rows `rows` of a readback to `out`, as RGBA without padding.
pub fn copy_rows<R>(
    renderer: &mut R,
    readback: &Readback<R::TextureMapping>,
    rows: Range<u32>,
    out: &mut Vec<u8>,
) -> Result<(), String>
where
    R: ExportMem,
{
    let bytes = renderer
        .map_texture(&readback.mapping)
        .map_err(|e| format!("map: {e}"))?;
    let row = readback.width as usize * BPP;
    let stride = bytes.len() / readback.height.max(1) as usize;
    if stride < row {
        return Err(format!("readback stride {stride} below row {row}"));
    }
    let rows = rows.start.min(readback.height)..rows.end.min(readback.height);
    if stride == row {
        out.extend_from_slice(&bytes[rows.start as usize * row..rows.end as usize * row]);
    } else {
        for y in rows {
            let start = y as usize * stride;
            out.extend_from_slice(&bytes[start..start + row]);
        }
    }
    Ok(())
}

/// Builds the image of a readback from its copied rows.
pub fn into_image<M>(readback: &Readback<M>, data: Vec<u8>) -> Result<Image, String> {
    Image::from_rgba(readback.width, readback.height, data).map_err(|e| format!("readback: {e}"))
}
