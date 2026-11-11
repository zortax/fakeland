//! Resolution of symbolic positions to points in the global logical space. Window positions use
//! the window geometry at the moment the event runs.

use fakeland_scenario::Position;
use fakeland_types::OutputId;
use smithay::utils::{Logical, Point};

use super::InjectError;
use crate::server::State;

impl State {
    /// Resolves a position to a global point and the output it lies on.
    pub fn resolve_position(
        &self,
        pos: &Position,
    ) -> Result<(Point<f64, Logical>, OutputId), InjectError> {
        if !pos.is_finite() {
            return Err(InjectError::NotFinite);
        }
        match pos {
            Position::Output { output, x, y } => self.output_point(*output, *x, *y),
            Position::Pixels {
                output,
                x,
                y,
                scale,
            } => self.output_point(*output, x * scale, y * scale),
            Position::Window { window, x, y } => {
                let id = self.resolve_window(window)?.ok_or(InjectError::NoWindow)?;
                let rec = self.windows.get(id).ok_or(InjectError::NoWindow)?;
                let info = self.window_info(rec);
                let g = info.geometry;
                Ok((Point::from((g.x as f64 + x, g.y as f64 + y)), info.output))
            }
            Position::WindowFraction { window, fx, fy } => {
                let id = self.resolve_window(window)?.ok_or(InjectError::NoWindow)?;
                let rec = self.windows.get(id).ok_or(InjectError::NoWindow)?;
                let info = self.window_info(rec);
                let g = info.geometry;
                Ok((
                    Point::from((g.x as f64 + fx * g.w as f64, g.y as f64 + fy * g.h as f64)),
                    info.output,
                ))
            }
        }
    }

    /// Resolves a position to output coordinates, so positions of different frames interpolate.
    pub fn position_on_output(&self, pos: &Position) -> Result<Position, InjectError> {
        let (point, output) = self.resolve_position(pos)?;
        let rect = self
            .outputs
            .get(output)
            .ok_or(InjectError::NoOutput)?
            .logical_rect();
        Ok(Position::on_output(
            output,
            point.x - rect.x as f64,
            point.y - rect.y as f64,
        ))
    }

    /// Returns the output that contains a global point.
    pub fn output_at(&self, point: Point<f64, Logical>) -> Option<OutputId> {
        self.outputs
            .iter()
            .find(|o| {
                let r = o.logical_rect();
                point.x >= r.x as f64
                    && point.y >= r.y as f64
                    && point.x < (r.x + r.w) as f64
                    && point.y < (r.y + r.h) as f64
            })
            .map(|o| o.id)
    }

    fn output_point(
        &self,
        output: Option<OutputId>,
        x: f64,
        y: f64,
    ) -> Result<(Point<f64, Logical>, OutputId), InjectError> {
        let id = match output {
            Some(id) => self.outputs.get(id).map(|o| o.id),
            None => self
                .output_at(self.windows.cursor.position)
                .or_else(|| self.outputs.primary()),
        }
        .ok_or(InjectError::NoOutput)?;
        let rect = self
            .outputs
            .get(id)
            .ok_or(InjectError::NoOutput)?
            .logical_rect();
        Ok((Point::from((rect.x as f64 + x, rect.y as f64 + y)), id))
    }
}
