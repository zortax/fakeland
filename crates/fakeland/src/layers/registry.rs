//! The layer surface registry: one record per `zwlr_layer_surface_v1` in creation order, and the
//! layer surface that took keyboard focus on demand.

use fakeland_types::{OutputId, ProcessId, SurfaceId, Timestamp, WindowId};
use smithay::desktop::LayerSurface;

/// One layer surface.
#[derive(Debug, Clone)]
pub struct LayerRecord {
    /// The window id.
    pub id: WindowId,
    /// The smithay layer surface.
    pub layer: LayerSurface,
    /// The root surface.
    pub surface: SurfaceId,
    /// The output that shows the surface.
    pub output: OutputId,
    /// The time of the first buffer, while mapped.
    pub mapped_at: Option<Timestamp>,
    /// The configure of the current mapping cycle is out. An unmap starts a new cycle.
    pub configured: bool,
    /// The client pid.
    pub pid: Option<ProcessId>,
    /// The launched process that the client descends from.
    pub app: Option<ProcessId>,
}

/// Every layer surface.
#[derive(Debug, Default)]
pub struct Layers {
    list: Vec<LayerRecord>,
    /// The layer surface that took keyboard focus on demand.
    pub focus: Option<WindowId>,
}

impl Layers {
    /// Adds a layer surface.
    pub fn insert(&mut self, record: LayerRecord) {
        self.list.push(record);
    }

    /// Removes a layer surface.
    pub fn remove(&mut self, id: WindowId) -> Option<LayerRecord> {
        let i = self.list.iter().position(|l| l.id == id)?;
        if self.focus == Some(id) {
            self.focus = None;
        }
        Some(self.list.remove(i))
    }

    /// Returns a layer surface.
    pub fn get(&self, id: WindowId) -> Option<&LayerRecord> {
        self.list.iter().find(|l| l.id == id)
    }

    /// Returns a layer surface for changes.
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut LayerRecord> {
        self.list.iter_mut().find(|l| l.id == id)
    }

    /// Returns the layer surface of a root surface.
    pub fn by_surface(&self, surface: SurfaceId) -> Option<WindowId> {
        self.list
            .iter()
            .find(|l| l.surface == surface)
            .map(|l| l.id)
    }

    /// Returns every layer surface in creation order.
    pub fn iter(&self) -> impl DoubleEndedIterator<Item = &LayerRecord> {
        self.list.iter()
    }

    /// Returns the ids of the layer surfaces on an output.
    pub fn on_output(&self, output: OutputId) -> Vec<WindowId> {
        self.list
            .iter()
            .filter(|l| l.output == output)
            .map(|l| l.id)
            .collect()
    }
}
