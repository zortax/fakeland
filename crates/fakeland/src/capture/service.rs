//! The capture service on the compositor thread: target resolution, the synchronous capture with
//! its downscale and cost record, and the composite of `Composite::EveryFrame`, which renders
//! without readback.

use fakeland_scenario::{CaptureTarget, Roi, WindowRef};
use fakeland_trace::Event;
use fakeland_types::{Logical, OutputId, Rect, WindowId};
use smithay::desktop::Window;
use smithay::utils::Point;

use super::{CaptureError, CaptureOptions, Screenshot};
use crate::server::State;

/// What the region shows.
#[derive(Clone)]
pub(super) enum Scene {
    /// Every window in the region.
    Space,
    /// One window alone, with its geometry origin.
    Window(Window, Point<i32, smithay::utils::Logical>),
}

/// A resolved capture.
pub(super) struct Plan {
    pub(super) region: Rect<Logical>,
    pub(super) scale: f64,
    pub(super) scene: Scene,
    pub(super) output: Option<OutputId>,
    pub(super) window: Option<WindowId>,
    pub(super) what: String,
}

impl State {
    /// Captures a target at once. The call blocks the compositor thread until the GPU is done and
    /// the rows are copied; [`State::capture_then`] runs the same capture in steps.
    pub fn capture(
        &mut self,
        target: &CaptureTarget,
        options: CaptureOptions,
    ) -> Result<Screenshot, CaptureError> {
        let start = self.now();
        let plan = self.plan_capture(target)?;
        let image = self
            .render_plan(&plan, options.include_cursor, true)?
            .ok_or(CaptureError::Empty)?;
        let end = self.now();
        let shot = Screenshot {
            image,
            region: plan.region,
            scale: 1.0 / plan.scale,
            output: plan.output,
            window: plan.window,
            time: end,
        };
        let shot = shot.downscaled(options.max_dim);
        let cost = self.now().since(start);
        self.emit(Event::Capture {
            what: plan.what,
            cost,
        });
        Ok(shot)
    }

    /// Composites an output without readback.
    pub fn composite_output(&mut self, output: OutputId) {
        let start = self.now();
        let target = CaptureTarget::Output {
            output: Some(output),
        };
        let result = self
            .plan_capture(&target)
            .and_then(|plan| self.render_plan(&plan, false, false));
        if let Err(e) = result {
            tracing::debug!("composite failed: {e}");
            return;
        }
        let end = self.now();
        self.emit_at(
            end,
            Event::CompositorOverhead {
                what: "composite".into(),
                duration: end.since(start),
            },
        );
    }

    fn resolve_window_ref(&self, r: &WindowRef) -> Result<WindowId, CaptureError> {
        self.resolve_window(r)?.ok_or(CaptureError::NoWindow)
    }

    pub(super) fn plan_capture(&self, target: &CaptureTarget) -> Result<Plan, CaptureError> {
        match target {
            CaptureTarget::Output { output } => {
                let id = self
                    .outputs
                    .resolve(*output)
                    .ok_or(CaptureError::NoOutput)?;
                let out = self.outputs.get(id).ok_or(CaptureError::NoOutput)?;
                Ok(Plan {
                    region: out.logical_rect(),
                    scale: out.config.scale,
                    scene: Scene::Space,
                    output: Some(id),
                    window: None,
                    what: format!("output {}", out.config.name),
                })
            }
            CaptureTarget::Window { window } => {
                let id = self.resolve_window_ref(window)?;
                self.plan_window(id, None)
            }
            CaptureTarget::Roi { roi } => match roi {
                Roi::Output { output, rect } => {
                    let id = self
                        .outputs
                        .resolve(*output)
                        .ok_or(CaptureError::NoOutput)?;
                    let out = self.outputs.get(id).ok_or(CaptureError::NoOutput)?;
                    let full = out.logical_rect();
                    let region = match rect {
                        Some(r) => r
                            .translate(full.x, full.y)
                            .intersection(full)
                            .ok_or(CaptureError::Empty)?,
                        None => full,
                    };
                    Ok(Plan {
                        region,
                        scale: out.config.scale,
                        scene: Scene::Space,
                        output: Some(id),
                        window: None,
                        what: format!("region of {}", out.config.name),
                    })
                }
                Roi::Window { window, rect } => {
                    let id = self.resolve_window_ref(window)?;
                    self.plan_window(id, *rect)
                }
            },
        }
    }

    fn plan_window(&self, id: WindowId, rect: Option<Rect<Logical>>) -> Result<Plan, CaptureError> {
        let rec = self.windows.get(id).ok_or(CaptureError::NoWindow)?;
        if rec.mapped_at.is_none() {
            return Err(CaptureError::NoWindow);
        }
        let geo = self
            .space
            .element_geometry(&rec.window)
            .ok_or(CaptureError::NoWindow)?;
        let full = Rect::new(geo.loc.x, geo.loc.y, geo.size.w, geo.size.h);
        let region = match rect {
            Some(r) => r
                .translate(full.x, full.y)
                .intersection(full)
                .ok_or(CaptureError::Empty)?,
            None => full,
        };
        let scale = self
            .outputs
            .get(rec.output)
            .map(|o| o.config.scale)
            .unwrap_or(1.0);
        Ok(Plan {
            region,
            scale,
            scene: Scene::Window(rec.window.clone(), geo.loc),
            output: Some(rec.output),
            window: Some(id),
            what: format!("window {}", id.0),
        })
    }
}
