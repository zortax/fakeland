//! The stacking order of an output: overlay and top layers above the toplevels, bottom and
//! background layers below them, and the lock surfaces alone while the session is locked. Hit
//! tests walk the same order and honor the input region of every surface.

use fakeland_types::{OutputId, WindowId};
use smithay::desktop::{LayerSurface, WindowSurfaceType, layer_map_for_output};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{IsAlive, Logical, Point};

use super::convert::shell_layer;
use crate::server::State;
use crate::shell::ShellLayer;

/// One mapped layer surface with its global origin.
#[derive(Debug, Clone)]
pub struct StackLayer {
    /// The window id.
    pub id: WindowId,
    /// The smithay layer surface.
    pub layer: LayerSurface,
    /// The origin of the root surface in the global logical space.
    pub origin: Point<i32, Logical>,
    /// The layer.
    pub level: ShellLayer,
}

/// The surface under a point.
#[derive(Debug, Clone)]
pub struct Hit {
    /// The window, layer surface or lock surface that owns the surface.
    pub window: WindowId,
    /// The surface.
    pub surface: WlSurface,
    /// The origin of the surface in the global logical space.
    pub origin: Point<f64, Logical>,
}

impl State {
    /// Returns the origin of a layer surface on its output in the global logical space.
    pub fn layer_origin(&self, id: WindowId) -> Option<Point<i32, Logical>> {
        let rec = self.layers.get(id)?;
        let out = self.outputs.get(rec.output)?;
        let geometry = layer_map_for_output(&out.output).layer_geometry(&rec.layer)?;
        let local = geometry.loc - rec.layer.bbox().loc;
        Some(Point::from((
            out.position.x + local.x,
            out.position.y + local.y,
        )))
    }

    /// Returns the mapped layer surfaces of an output with global origins, bottom to top.
    pub fn layer_stack(&self, output: OutputId) -> Vec<StackLayer> {
        let mut list: Vec<StackLayer> = self
            .layers
            .iter()
            .filter(|l| l.output == output && l.mapped_at.is_some() && l.layer.alive())
            .filter_map(|l| {
                Some(StackLayer {
                    id: l.id,
                    layer: l.layer.clone(),
                    origin: self.layer_origin(l.id)?,
                    level: shell_layer(l.layer.layer()),
                })
            })
            .collect();
        list.sort_by_key(|l| l.level);
        list
    }

    /// Returns the mapped layer surfaces of every output, bottom to top within each output.
    pub fn all_layers(&self) -> Vec<StackLayer> {
        let outputs: Vec<OutputId> = self.outputs.iter().map(|o| o.id).collect();
        outputs
            .into_iter()
            .flat_map(|o| self.layer_stack(o))
            .collect()
    }

    /// Returns the topmost surface that accepts input at a global point.
    pub fn hit(&self, point: Point<f64, Logical>) -> Option<Hit> {
        if self.lock.locked {
            return self.lock_hit(point);
        }
        let stack = self
            .output_at(point)
            .map(|o| self.layer_stack(o))
            .unwrap_or_default();
        let (above, below): (Vec<&StackLayer>, Vec<&StackLayer>) =
            stack.iter().partition(|l| l.level >= ShellLayer::Top);
        layer_hit(above.into_iter().rev(), point)
            .or_else(|| self.toplevel_hit(point))
            .or_else(|| layer_hit(below.into_iter().rev(), point))
    }

    fn toplevel_hit(&self, point: Point<f64, Logical>) -> Option<Hit> {
        let (window, loc) = self.space.element_under(point)?;
        let (surface, at) = window.surface_under(point - loc.to_f64(), WindowSurfaceType::ALL)?;
        let id = self.windows.iter().find(|w| w.window == *window)?.id;
        Some(Hit {
            window: id,
            surface,
            origin: (at + loc).to_f64(),
        })
    }
}

/// Returns the first layer surface, topmost first, that accepts input at a global point.
fn layer_hit<'a>(
    layers: impl Iterator<Item = &'a StackLayer>,
    point: Point<f64, Logical>,
) -> Option<Hit> {
    layers.into_iter().find_map(|l| {
        let (surface, at) = l
            .layer
            .surface_under(point - l.origin.to_f64(), WindowSurfaceType::ALL)?;
        Some(Hit {
            window: l.id,
            surface,
            origin: (at + l.origin).to_f64(),
        })
    })
}
