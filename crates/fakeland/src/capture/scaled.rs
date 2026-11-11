//! Captures that the GPU scales: the region renders at a fraction of the output scale, so the
//! readback holds only the pixels of the smaller picture.

use fakeland_scenario::CaptureTarget;
use fakeland_trace::Event;
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::reexports::wayland_server::Resource;
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::with_states;

use super::{CaptureError, Screenshot};
use crate::server::State;

impl State {
    /// Returns the hotspot of the client cursor surface, or zero for the built-in cursor.
    pub fn cursor_hotspot(&self) -> Point<i32, Logical> {
        match &self.windows.cursor.image {
            CursorImageStatus::Surface(s) if s.is_alive() => with_states(s, |states| {
                states
                    .data_map
                    .get::<CursorImageSurfaceData>()
                    .and_then(|d| d.lock().ok().map(|a| a.hotspot))
                    .unwrap_or_default()
            }),
            _ => Point::default(),
        }
    }

    /// Returns the pixel size of a capture at `factor` times the output scale.
    pub fn capture_size(
        &self,
        target: &CaptureTarget,
        factor: f64,
    ) -> Result<(u32, u32), CaptureError> {
        let plan = self.plan_capture(target)?;
        let scale = plan.scale * factor;
        let px = |v: i32| ((f64::from(v) * scale).round().max(1.0)) as u32;
        Ok((px(plan.region.w), px(plan.region.h)))
    }

    /// Captures a target rendered at `factor` times the output scale. A factor of 1 equals a
    /// plain capture; a factor below 1 makes the GPU scale the picture down.
    pub fn capture_scaled(
        &mut self,
        target: &CaptureTarget,
        include_cursor: bool,
        factor: f64,
    ) -> Result<Screenshot, CaptureError> {
        let start = self.now();
        let mut plan = self.plan_capture(target)?;
        plan.scale *= factor.clamp(1e-3, 1.0);
        let image = self
            .render_plan(&plan, include_cursor, true)?
            .ok_or(CaptureError::Empty)?;
        let end = self.now();
        self.emit_at(
            end,
            Event::Capture {
                what: plan.what.clone(),
                cost: end.since(start),
            },
        );
        Ok(Screenshot {
            image,
            region: plan.region,
            scale: 1.0 / plan.scale,
            output: plan.output,
            window: plan.window,
            time: end,
        })
    }
}
