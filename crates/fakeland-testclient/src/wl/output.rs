//! wl_output scales and the outputs the surface is on. The largest scale of the entered outputs
//! is the integer scale hint for compositors without wl_surface.preferred_buffer_scale.

use std::collections::{HashMap, HashSet};

use wayland_client::backend::ObjectId;
use wayland_client::globals::GlobalList;
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use crate::app::App;

/// The highest wl_output version the client binds.
const OUTPUT_VERSION: u32 = 4;

/// The bound outputs and their scales.
#[derive(Debug, Default)]
pub struct Outputs {
    bound: Vec<WlOutput>,
    scales: HashMap<ObjectId, i32>,
    entered: HashSet<ObjectId>,
}

impl Outputs {
    /// Binds every output of the initial registry list.
    pub fn bind_all(&mut self, globals: &GlobalList, qh: &QueueHandle<App>) {
        let registry = globals.registry();
        for g in globals.contents().clone_list() {
            if g.interface == WlOutput::interface().name {
                self.bind(registry, g.name, g.version, qh);
            }
        }
    }

    /// Binds one output global.
    pub fn bind(&mut self, registry: &WlRegistry, name: u32, version: u32, qh: &QueueHandle<App>) {
        let output: WlOutput = registry.bind(name, version.min(OUTPUT_VERSION), qh, ());
        self.scales.insert(output.id(), 1);
        self.bound.push(output);
    }

    /// Records the scale of an output.
    pub fn set_scale(&mut self, output: ObjectId, scale: i32) {
        self.scales.insert(output, scale.max(1));
    }

    /// Records that the surface entered an output.
    pub fn enter(&mut self, output: ObjectId) {
        self.entered.insert(output);
    }

    /// Records that the surface left an output.
    pub fn leave(&mut self, output: &ObjectId) {
        self.entered.remove(output);
    }

    /// Returns the largest scale of the entered outputs, or 1.
    pub fn entered_scale(&self) -> i32 {
        self.entered
            .iter()
            .filter_map(|id| self.scales.get(id))
            .copied()
            .max()
            .unwrap_or(1)
    }
}

impl Dispatch<WlOutput, ()> for App {
    fn event(
        app: &mut Self,
        output: &WlOutput,
        event: wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Scale { factor } = event {
            app.outputs.set_scale(output.id(), factor);
            let scale = app.outputs.entered_scale();
            app.scale.set_outputs(scale);
            app.scale_hint_changed();
        }
    }
}
