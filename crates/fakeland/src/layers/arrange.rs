//! The layout of the layer surfaces of an output: anchors, margins, sizes and exclusive zones.
//! The zones that stay free form the work area of the toplevels, and a change of the work area
//! moves the toplevels of the output.

use fakeland_types::{Logical, OutputId, Rect};
use smithay::desktop::layer_map_for_output;

use crate::server::State;

impl State {
    /// Arranges the layer surfaces of an output. A changed work area moves the toplevels of the
    /// output.
    pub fn arrange_layers(&mut self, output: OutputId) {
        let Some(out) = self.outputs.get(output) else {
            return;
        };
        let (changed, before, after) = {
            let mut map = layer_map_for_output(&out.output);
            let before = map.non_exclusive_zone();
            let changed = map.arrange();
            (changed, before, map.non_exclusive_zone())
        };
        if changed {
            self.space_dirty = true;
        }
        if before != after {
            for w in self.windows.on_output(output) {
                self.relayout_window(w);
            }
        }
    }

    /// Returns the work area of an output in the global logical space: the output without the
    /// exclusive zones of its layer surfaces.
    pub fn work_area(&self, output: OutputId) -> Option<Rect<Logical>> {
        let out = self.outputs.get(output)?;
        let full = out.logical_rect();
        let zone = layer_map_for_output(&out.output).non_exclusive_zone();
        if zone.size.w <= 0 || zone.size.h <= 0 {
            return Some(full);
        }
        let area = Rect::new(
            full.x + zone.loc.x,
            full.y + zone.loc.y,
            zone.size.w,
            zone.size.h,
        );
        Some(area.intersection(full).unwrap_or(full))
    }
}
