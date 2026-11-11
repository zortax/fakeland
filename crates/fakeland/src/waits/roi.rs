//! Resolution of regions of interest: the output, the window and the rectangle in the global
//! logical space.

use fakeland_scenario::Roi;
use fakeland_types::{Logical, OutputId, Rect, WindowId};

use crate::capture::CaptureError;
use crate::server::State;

/// A region of interest resolved against the current layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoiTarget {
    /// The output that shows the region.
    pub output: OutputId,
    /// The window of a window region.
    pub window: Option<WindowId>,
    /// The region in the global logical space.
    pub rect: Rect<Logical>,
}

impl State {
    /// Resolves a region of interest.
    pub fn resolve_roi(&self, roi: &Roi) -> Result<RoiTarget, CaptureError> {
        match roi {
            Roi::Output { output, rect } => {
                let id = self
                    .outputs
                    .resolve(*output)
                    .ok_or(CaptureError::NoOutput)?;
                let full = self
                    .outputs
                    .get(id)
                    .ok_or(CaptureError::NoOutput)?
                    .logical_rect();
                let rect = match rect {
                    Some(r) => r
                        .translate(full.x, full.y)
                        .intersection(full)
                        .ok_or(CaptureError::Empty)?,
                    None => full,
                };
                Ok(RoiTarget {
                    output: id,
                    window: None,
                    rect,
                })
            }
            Roi::Window { window, rect } => {
                let id = self.resolve_window(window)?.ok_or(CaptureError::NoWindow)?;
                let rec = self.windows.get(id).ok_or(CaptureError::NoWindow)?;
                let full = self.window_info(rec).geometry;
                let rect = match rect {
                    Some(r) => r
                        .translate(full.x, full.y)
                        .intersection(full)
                        .ok_or(CaptureError::Empty)?,
                    None => full,
                };
                Ok(RoiTarget {
                    output: rec.output,
                    window: Some(id),
                    rect,
                })
            }
        }
    }

    /// Returns a region pinned to the window id it resolves to now, so later captures follow the
    /// same window.
    pub fn pin_roi(&self, roi: &Roi) -> Result<Roi, CaptureError> {
        Ok(match roi {
            Roi::Window { window, rect } => Roi::Window {
                window: fakeland_scenario::WindowRef::id(
                    self.resolve_window(window)?.ok_or(CaptureError::NoWindow)?,
                ),
                rect: *rect,
            },
            Roi::Output { output, rect } => Roi::Output {
                output: Some(
                    self.outputs
                        .resolve(*output)
                        .ok_or(CaptureError::NoOutput)?,
                ),
                rect: *rect,
            },
        })
    }
}
