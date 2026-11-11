//! The render of a capture plan on the session renderer. The render uploads the shm buffers that
//! have no texture yet, draws into a reused offscreen target and starts the readback; the rows
//! then come back in any number of chunks.

use std::ops::Range;

use fakeland_scenario::CaptureTarget;
use fakeland_types::Nanos;
use fakeland_vision::Image;
use smithay::backend::renderer::gles::{GlesRenderbuffer, GlesRenderer, GlesTexture};
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::sync::SyncPoint;
use smithay::backend::renderer::utils::RendererSurfaceStateUserData;
use smithay::backend::renderer::{ExportMem, Renderer};
use smithay::desktop::Window;
use smithay::reexports::pixman;
use smithay::utils::{Physical, Point, Rectangle, Size};
use smithay::wayland::shm::with_buffer_contents;

use super::CaptureError;
use super::service::{Plan, Scene};
use crate::render::{
    CursorScene, Readback, RenderBackend, WindowScene, collect, copy_rows, create_target,
    into_image, map, render,
};
use crate::server::State;

/// A capture readback in flight on the session renderer.
pub(super) enum Pending {
    /// A readback of the GLES renderer.
    Gles(Readback<<GlesRenderer as ExportMem>::TextureMapping>),
    /// A readback of the Pixman renderer.
    Pixman(Readback<<PixmanRenderer as ExportMem>::TextureMapping>),
}

impl Pending {
    /// Returns the size in pixels.
    pub(super) fn size(&self) -> (u32, u32) {
        match self {
            Self::Gles(r) => (r.width(), r.height()),
            Self::Pixman(r) => (r.width(), r.height()),
        }
    }

    /// Returns the fence of the readback.
    pub(super) fn fence(&self) -> &SyncPoint {
        match self {
            Self::Gles(r) => r.fence(),
            Self::Pixman(r) => r.fence(),
        }
    }

    /// Builds the image from the copied rows.
    pub(super) fn into_image(self, data: Vec<u8>) -> Result<Image, CaptureError> {
        match &self {
            Self::Gles(r) => into_image(r, data),
            Self::Pixman(r) => into_image(r, data),
        }
        .map_err(CaptureError::Render)
    }
}

/// Returns the pixel size of a plan.
pub(super) fn plan_size(plan: &Plan) -> Size<i32, Physical> {
    (
        ((plan.region.w as f64) * plan.scale).round().max(1.0) as i32,
        ((plan.region.h as f64) * plan.scale).round().max(1.0) as i32,
    )
        .into()
}

/// A render that started.
pub(super) struct Started {
    /// The readback, when the render reads back.
    pub(super) pending: Option<Pending>,
    /// The time of the element collection, which uploads shm buffers.
    pub(super) upload: Nanos,
}

impl State {
    /// Returns the pixel count of the shm buffers in the scene of a plan that have no GLES
    /// texture. The render step of a capture uploads them.
    pub(super) fn pending_upload_pixels(&mut self, plan: &Plan) -> u64 {
        let RenderBackend::Gles(dev) = &mut self.render else {
            return 0;
        };
        let Some(context) = dev.renderer().map(|r| r.context_id()) else {
            return 0;
        };
        let region = Rectangle::new(
            (plan.region.x, plan.region.y).into(),
            (plan.region.w, plan.region.h).into(),
        );
        let windows: Vec<Window> = match &plan.scene {
            Scene::Space => self
                .space
                .elements()
                .filter(|w| {
                    self.space
                        .element_bbox(w)
                        .is_some_and(|bbox| bbox.overlaps(region))
                })
                .cloned()
                .collect(),
            Scene::Window(w, _) => vec![w.clone()],
        };
        let mut pixels = 0;
        for window in &windows {
            window.with_surfaces(|_, states| {
                let Some(data) = states.data_map.get::<RendererSurfaceStateUserData>() else {
                    return;
                };
                let Ok(state) = data.lock() else { return };
                let Some(buffer) = state.buffer() else { return };
                if state.texture::<GlesTexture>(context.clone()).is_some() {
                    return;
                }
                if let Ok((w, h)) = with_buffer_contents(buffer, |_, _, d| (d.width, d.height)) {
                    pixels += w.max(0) as u64 * h.max(0) as u64;
                }
            });
        }
        pixels
    }

    /// Renders a plan and starts its readback when `readback` is set.
    pub(super) fn start_plan(
        &mut self,
        plan: &Plan,
        include_cursor: bool,
        readback: bool,
    ) -> Result<Started, CaptureError> {
        if plan.region.is_empty() {
            return Err(CaptureError::Empty);
        }
        let region = Rectangle::new(
            (plan.region.x, plan.region.y).into(),
            (plan.region.w, plan.region.h).into(),
        );
        let size = plan_size(plan);
        let read = readback.then(|| Rectangle::from_size(size));
        let hotspot = self.cursor_hotspot();
        let cursor = include_cursor.then(|| CursorScene {
            position: self.windows.cursor.position,
            image: &self.windows.cursor.image,
            hotspot,
            default: &self.windows.cursor.default,
            default_hotspot: Point::default(),
        });
        let scene = match &plan.scene {
            Scene::Space => WindowScene::Space(&self.space),
            Scene::Window(w, loc) => WindowScene::Single(w, *loc),
        };
        let targets = &mut self.capture_targets;
        let clock = self.clock.clone();
        let upload;
        let result = match &mut self.render {
            RenderBackend::Gles(dev) => {
                let renderer = dev
                    .renderer()
                    .ok_or_else(|| CaptureError::Render("no GLES renderer".into()))?;
                let start = clock.now();
                let elements = collect(renderer, &scene, region, plan.scale, cursor.as_ref());
                upload = clock.now().since(start);
                let mut target: GlesRenderbuffer = targets
                    .gles
                    .take(size, || create_target(renderer, size))
                    .map_err(CaptureError::Render)?;
                let r = render(renderer, &mut target, &elements, size, plan.scale, read);
                targets.gles.put(size, target);
                r.map(|r| r.map(Pending::Gles))
            }
            RenderBackend::Pixman(renderer) => {
                let start = clock.now();
                let elements = collect(renderer, &scene, region, plan.scale, cursor.as_ref());
                upload = clock.now().since(start);
                let mut target: pixman::Image<'static, 'static> = targets
                    .pixman
                    .take(size, || create_target(renderer, size))
                    .map_err(CaptureError::Render)?;
                let r = render(renderer, &mut target, &elements, size, plan.scale, read);
                targets.pixman.put(size, target);
                r.map(|r| r.map(Pending::Pixman))
            }
        };
        let pending = result.map_err(CaptureError::Render)?;
        Ok(Started { pending, upload })
    }

    /// Maps the pack buffer of a readback into memory.
    pub(super) fn map_pending(&mut self, pending: &Pending) -> Result<(), CaptureError> {
        let result = match (&mut self.render, pending) {
            (RenderBackend::Gles(dev), Pending::Gles(r)) => match dev.renderer() {
                Some(renderer) => map(renderer, r),
                None => Err("no GLES renderer".into()),
            },
            (RenderBackend::Pixman(renderer), Pending::Pixman(r)) => map(renderer, r),
            _ => Err("the readback belongs to another renderer".into()),
        };
        result.map_err(CaptureError::Render)
    }

    /// Appends the rows `rows` of a readback to `out`.
    pub(super) fn copy_pending(
        &mut self,
        pending: &Pending,
        rows: Range<u32>,
        out: &mut Vec<u8>,
    ) -> Result<(), CaptureError> {
        let result = match (&mut self.render, pending) {
            (RenderBackend::Gles(dev), Pending::Gles(r)) => match dev.renderer() {
                Some(renderer) => copy_rows(renderer, r, rows, out),
                None => Err("no GLES renderer".into()),
            },
            (RenderBackend::Pixman(renderer), Pending::Pixman(r)) => {
                copy_rows(renderer, r, rows, out)
            }
            _ => Err("the readback belongs to another renderer".into()),
        };
        result.map_err(CaptureError::Render)
    }

    /// Renders and maps a readback of every output with the cursor before any client connects.
    /// The renderer then has its shaders, its readback buffers and a target of each output size
    /// before the first real capture.
    pub fn warm_up_capture(&mut self) {
        let outputs: Vec<_> = self.outputs.iter().map(|o| o.id).collect();
        for output in outputs {
            let target = CaptureTarget::Output {
                output: Some(output),
            };
            let result = self.plan_capture(&target).and_then(|plan| {
                let Some(pending) = self.start_plan(&plan, true, true)?.pending else {
                    return Ok(());
                };
                pending
                    .fence()
                    .wait()
                    .map_err(|e| CaptureError::Render(format!("fence: {e:?}")))?;
                self.map_pending(&pending)
            });
            if let Err(e) = result {
                tracing::debug!("capture warm-up failed: {e}");
            }
        }
    }

    /// Renders a plan and reads it back at once. The call blocks until the GPU is done.
    pub(super) fn render_plan(
        &mut self,
        plan: &Plan,
        include_cursor: bool,
        readback: bool,
    ) -> Result<Option<Image>, CaptureError> {
        let Some(pending) = self.start_plan(plan, include_cursor, readback)?.pending else {
            return Ok(None);
        };
        let (w, h) = pending.size();
        let mut data = Vec::with_capacity(w as usize * h as usize * 4);
        self.copy_pending(&pending, 0..h, &mut data)?;
        pending.into_image(data).map(Some)
    }
}
