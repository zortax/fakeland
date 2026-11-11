//! The compositor windows as the accessibility client sees them: pid, title, window geometry,
//! surface bounding box and output scale. Global points map back to output positions for pointer
//! input.

use fakeland_a11y::WindowFrame;
use fakeland_scenario::Position;
use fakeland_types::{Logical, OutputId, Point, Rect};

use crate::server::State;

impl State {
    /// Returns every mapped window as an accessibility window frame.
    pub fn a11y_windows(&self) -> Vec<WindowFrame> {
        self.window_infos()
            .into_iter()
            .filter(|w| w.is_mapped())
            .filter_map(|info| {
                let rec = self.windows.get(info.id)?;
                let geo = rec.window.geometry();
                let bbox = rec.window.bbox();
                let origin_x = info.geometry.x - geo.loc.x;
                let origin_y = info.geometry.y - geo.loc.y;
                let scale = self
                    .outputs
                    .get(info.output)
                    .map(|o| o.config.scale)
                    .unwrap_or(1.0);
                Some(WindowFrame {
                    id: info.id,
                    pid: info.pid,
                    app: info.app,
                    title: info.title,
                    app_id: info.app_id,
                    geometry: info.geometry,
                    surface: Rect::new(
                        origin_x + bbox.loc.x,
                        origin_y + bbox.loc.y,
                        bbox.size.w,
                        bbox.size.h,
                    ),
                    scale,
                    focused: info.focused,
                })
            })
            .collect()
    }

    /// Returns the output position of a global logical point: the output that contains it, else
    /// the fallback output, else the primary output.
    pub fn global_to_position(&self, p: Point<Logical>, fallback: Option<OutputId>) -> Position {
        let hit = self
            .outputs
            .iter()
            .filter(|o| o.config.enabled)
            .find(|o| o.logical_rect().contains(p))
            .map(|o| o.id);
        let output = hit.or(fallback).or_else(|| self.outputs.primary());
        let (ox, oy) = output
            .and_then(|id| self.outputs.get(id))
            .map(|o| (o.position.x as f64, o.position.y as f64))
            .unwrap_or((0.0, 0.0));
        Position::Output {
            output,
            x: p.x - ox,
            y: p.y - oy,
        }
    }
}
