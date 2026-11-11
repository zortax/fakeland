//! Window operations from the session: focus, size, placement, suspension, output and close.

use fakeland_scenario::{WindowAction, WindowRef};
use fakeland_types::{Logical, OutputId, Rect, Size, WindowId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{Placement, placement_rect};
use crate::server::State;

/// One operation on a window.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WindowOp {
    /// Gives the window keyboard focus and raises it.
    Focus,
    /// Floats the window at its position with a new size.
    Resize {
        /// The new size.
        size: Size<Logical>,
    },
    /// Maximizes the window on its output.
    Maximize,
    /// Makes the window fullscreen on its output.
    Fullscreen,
    /// Floats the window at a rectangle of the global space.
    Floating {
        /// The geometry.
        rect: Rect<Logical>,
    },
    /// Sets or clears the suspended state.
    Suspend {
        /// The new state.
        suspended: bool,
    },
    /// Moves the window to another output.
    MoveToOutput {
        /// The output.
        output: OutputId,
    },
    /// Asks the client to close the window.
    Close,
}

impl WindowOp {
    /// Converts a scenario window action. A resize storm is a timed program and has no single
    /// operation.
    pub fn from_action(action: &WindowAction) -> Option<(WindowRef, Self)> {
        let op = match action {
            WindowAction::Resize { size, .. } => Self::Resize { size: *size },
            WindowAction::ResizeStorm { .. } => return None,
            WindowAction::Focus { .. } => Self::Focus,
            WindowAction::Maximize { .. } => Self::Maximize,
            WindowAction::Fullscreen { .. } => Self::Fullscreen,
            WindowAction::Floating { rect, .. } => Self::Floating { rect: *rect },
            WindowAction::Suspend { suspended, .. } => Self::Suspend {
                suspended: *suspended,
            },
            WindowAction::Close { .. } => Self::Close,
        };
        Some((action.window().clone(), op))
    }
}

/// A window operation that cannot run.
#[derive(Debug, Error)]
pub enum WindowOpError {
    /// The window does not exist.
    #[error("no window {0}")]
    NoWindow(WindowId),
    /// The output does not exist.
    #[error("no output {0}")]
    NoOutput(OutputId),
    /// The size is not positive.
    #[error("window sizes must be positive")]
    Size,
}

impl State {
    /// Runs one window operation.
    pub fn window_op(&mut self, id: WindowId, op: WindowOp) -> Result<(), WindowOpError> {
        let rec = self.windows.get(id).ok_or(WindowOpError::NoWindow(id))?;
        let current = self
            .outputs
            .get(rec.output)
            .map(|o| placement_rect(rec.placement, o.logical_rect(), rec.floating))
            .unwrap_or(rec.floating);
        match op {
            WindowOp::Focus => {
                self.focus_window(Some(id));
                return Ok(());
            }
            WindowOp::Close => {
                rec.toplevel.send_close();
                return Ok(());
            }
            WindowOp::MoveToOutput { output } => {
                let to = self
                    .outputs
                    .get(output)
                    .ok_or(WindowOpError::NoOutput(output))?
                    .logical_rect();
                let from = self.outputs.get(rec.output).map(|o| o.logical_rect());
                let rec = self
                    .windows
                    .get_mut(id)
                    .ok_or(WindowOpError::NoWindow(id))?;
                if let Some(from) = from {
                    rec.floating = rec.floating.translate(to.x - from.x, to.y - from.y);
                }
                rec.output = output;
            }
            _ => {}
        }
        let rec = self
            .windows
            .get_mut(id)
            .ok_or(WindowOpError::NoWindow(id))?;
        match op {
            WindowOp::Resize { size } => {
                if size.w <= 0 || size.h <= 0 {
                    return Err(WindowOpError::Size);
                }
                rec.placement = Placement::Floating;
                rec.floating = Rect::new(current.x, current.y, size.w, size.h);
            }
            WindowOp::Floating { rect } => {
                if rect.w <= 0 || rect.h <= 0 {
                    return Err(WindowOpError::Size);
                }
                rec.placement = Placement::Floating;
                rec.floating = rect;
            }
            WindowOp::Maximize => rec.placement = Placement::Maximized,
            WindowOp::Fullscreen => rec.placement = Placement::Fullscreen,
            WindowOp::Suspend { suspended } => rec.suspended = suspended,
            WindowOp::Focus | WindowOp::Close | WindowOp::MoveToOutput { .. } => {}
        }
        self.relayout_window(id);
        Ok(())
    }
}
