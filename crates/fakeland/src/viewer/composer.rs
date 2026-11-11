//! The GPU path of the viewer: each output composites into a swapchain of GBM dmabufs with
//! damage tracking, and the viewer hands the buffers to the host as they are. A slot stays out of
//! the swapchain until the host releases it, so the composite never draws into a buffer that the
//! host still reads.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use fakeland_types::{OutputId, Rect as FRect};
use smithay::backend::allocator::dmabuf::{AsDmabuf, Dmabuf};
use smithay::backend::allocator::gbm::{GbmAllocator, GbmBuffer, GbmBufferFlags, GbmDevice};
use smithay::backend::allocator::{Fourcc, Modifier, Slot, Swapchain};
use smithay::backend::drm::DrmDeviceFd;
use smithay::backend::renderer::Bind;
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::utils::{Rectangle, Size, Transform};

use super::ViewerFrame;
use crate::render::{CursorScene, WindowScene, collect};
use crate::server::State;

/// The background of the composite.
const CLEAR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// Formats the composite renders into, in order of preference.
const PREFERRED: [Fourcc; 4] = [
    Fourcc::Xrgb8888,
    Fourcc::Argb8888,
    Fourcc::Xbgr8888,
    Fourcc::Abgr8888,
];

static NEXT_SLOT: AtomicU64 = AtomicU64::new(1);

/// The identity and buffer of a swapchain slot.
struct SlotData {
    id: u64,
    dmabuf: Dmabuf,
}

/// Picks the first preferred format that both the renderer draws into and the host imports,
/// with the modifiers both sides know.
pub fn choose_format(
    render: impl IntoIterator<Item = (u32, u64)>,
    host: &[(u32, u64)],
) -> Option<(Fourcc, Vec<Modifier>)> {
    let render: Vec<(u32, u64)> = render.into_iter().collect();
    PREFERRED.iter().find_map(|&fourcc| {
        let code = fourcc as u32;
        let mods: Vec<Modifier> = render
            .iter()
            .filter(|(c, m)| *c == code && host.contains(&(code, *m)))
            .map(|(_, m)| Modifier::from(*m))
            .collect();
        (!mods.is_empty()).then_some((fourcc, mods))
    })
}

/// The composite of one output into dmabufs.
pub struct Composer {
    swapchain: Swapchain<GbmAllocator<DrmDeviceFd>>,
    damage: OutputDamageTracker,
    region: FRect<fakeland_types::Logical>,
    scale: f64,
    held: HashMap<u64, Slot<GbmBuffer>>,
    ids: Vec<u64>,
}

impl std::fmt::Debug for Composer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Composer")
            .field("region", &self.region)
            .field("scale", &self.scale)
            .field("held", &self.held.len())
            .finish()
    }
}

impl Composer {
    /// Returns true when the composer matches the output region and scale.
    pub fn fits(&self, region: FRect<fakeland_types::Logical>, scale: f64) -> bool {
        self.region == region && (self.scale - scale).abs() < 1e-9
    }

    /// Returns every slot id the composer handed out.
    pub fn ids(&self) -> &[u64] {
        &self.ids
    }

    /// Gives a slot back to the swapchain.
    pub fn release(&mut self, id: u64) {
        self.held.remove(&id);
    }
}

impl State {
    /// Builds the composer of an output for the host formats, or returns why it cannot.
    pub fn viewer_composer(
        &mut self,
        output: OutputId,
        host: &[(u32, u64)],
    ) -> Result<Composer, String> {
        let crate::render::RenderBackend::Gles(dev) = &mut self.render else {
            return Err("the Pixman renderer has no dmabufs".into());
        };
        let out = self.outputs.get(output).ok_or("no such output")?;
        let region = out.logical_rect();
        let scale = out.config.scale;
        let renderer = dev.renderer().ok_or("no GLES renderer")?;
        let render: Vec<(u32, u64)> = renderer
            .egl_context()
            .dmabuf_render_formats()
            .iter()
            .map(|f| (f.code as u32, u64::from(f.modifier)))
            .collect();
        let (fourcc, modifiers) =
            choose_format(render, host).ok_or("no dmabuf format both sides know")?;
        let gbm = GbmDevice::new(dev.device_fd.clone()).map_err(|e| e.to_string())?;
        let allocator = GbmAllocator::new(gbm, GbmBufferFlags::RENDERING);
        let size = physical_size(region, scale);
        Ok(Composer {
            swapchain: Swapchain::new(allocator, size.w as u32, size.h as u32, fourcc, modifiers),
            damage: OutputDamageTracker::new(size, scale, Transform::Normal),
            region,
            scale,
            held: HashMap::new(),
            ids: Vec::new(),
        })
    }

    /// Composites an output into a free slot. Returns `None` when no slot is free or nothing
    /// changed since the slot was last drawn.
    pub fn viewer_compose(
        &mut self,
        composer: &mut Composer,
    ) -> Result<Option<ViewerFrame>, String> {
        let Some(slot) = composer.swapchain.acquire().map_err(|e| e.to_string())? else {
            return Ok(None);
        };
        slot.userdata().insert_if_missing_threadsafe(|| SlotData {
            id: NEXT_SLOT.fetch_add(1, Ordering::Relaxed),
            dmabuf: slot.export().expect("a GBM buffer exports as dmabuf"),
        });
        let data = slot
            .userdata()
            .get::<SlotData>()
            .ok_or("the slot has no data")?;
        let (id, mut dmabuf) = (data.id, data.dmabuf.clone());
        if !composer.ids.contains(&id) {
            composer.ids.push(id);
        }
        let age = usize::from(slot.age());
        let region = composer.region;
        let rect = Rectangle::new((region.x, region.y).into(), (region.w, region.h).into());
        let hotspot = self.cursor_hotspot();
        let cursor = CursorScene {
            position: self.windows.cursor.position,
            image: &self.windows.cursor.image,
            hotspot,
            default: &self.windows.cursor.default,
            default_hotspot: Default::default(),
        };
        let crate::render::RenderBackend::Gles(dev) = &mut self.render else {
            return Err("the Pixman renderer has no dmabufs".into());
        };
        let renderer = dev.renderer().ok_or("no GLES renderer")?;
        let elements = collect(
            renderer,
            &WindowScene::Space(&self.space),
            rect,
            composer.scale,
            Some(&cursor),
        );
        let mut framebuffer = renderer.bind(&mut dmabuf).map_err(|e| e.to_string())?;
        let result = composer
            .damage
            .render_output(renderer, &mut framebuffer, age, &elements, CLEAR)
            .map_err(|e| format!("{e:?}"))?;
        if result.damage.is_none() && age > 0 {
            return Ok(None);
        }
        let sync = result.sync;
        drop(framebuffer);
        composer.swapchain.submitted(&slot);
        composer.held.insert(id, slot);
        Ok(Some(ViewerFrame::Dmabuf { id, dmabuf, sync }))
    }
}

/// Returns the pixel size of a logical region at a scale.
pub fn physical_size(
    region: FRect<fakeland_types::Logical>,
    scale: f64,
) -> Size<i32, smithay::utils::Physical> {
    (
        ((f64::from(region.w)) * scale).round().max(1.0) as i32,
        ((f64::from(region.h)) * scale).round().max(1.0) as i32,
    )
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINEAR: u64 = 0;
    const NV: u64 = 0x0300_0000_0060_6014;

    #[test]
    fn picks_the_first_shared_format_and_modifiers() {
        let render = [
            (Fourcc::Argb8888 as u32, LINEAR),
            (Fourcc::Argb8888 as u32, NV),
            (Fourcc::Xrgb8888 as u32, NV),
        ];
        let host = [
            (Fourcc::Argb8888 as u32, LINEAR),
            (Fourcc::Argb8888 as u32, NV),
            (Fourcc::Xrgb8888 as u32, LINEAR),
        ];
        let (fourcc, mods) = choose_format(render, &host).unwrap();
        assert_eq!(fourcc, Fourcc::Argb8888);
        assert_eq!(mods, vec![Modifier::from(LINEAR), Modifier::from(NV)]);
        assert!(choose_format(render, &[]).is_none());
    }

    #[test]
    fn physical_size_rounds() {
        let s = physical_size(FRect::new(0, 0, 1280, 720), 1.5);
        assert_eq!((s.w, s.h), (1920, 1080));
    }
}
