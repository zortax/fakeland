//! Outputs: the binding of every `wl_output` global, their connector names, and hotplug. A new
//! output gets a lock surface while the client holds a lock.

use serde_json::json;
use wayland_client::globals::{GlobalList, GlobalListContents};
use wayland_client::protocol::wl_output::{self, WlOutput};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use crate::app::App;
use crate::log;

/// The highest `wl_output` version the client binds.
const OUTPUT_VERSION: u32 = 4;

/// Binds every output of the initial registry.
pub fn bind_outputs(globals: &GlobalList, qh: &QueueHandle<App>) -> Vec<WlOutput> {
    let registry = globals.registry();
    globals.contents().with_list(|list| {
        list.iter()
            .filter(|g| g.interface == WlOutput::interface().name)
            .map(|g| {
                registry.bind::<WlOutput, _, App>(g.name, g.version.min(OUTPUT_VERSION), qh, g.name)
            })
            .collect()
    })
}

impl Dispatch<WlRegistry, GlobalListContents> for App {
    fn event(
        app: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            wl_registry::Event::Global {
                name,
                interface,
                version,
            } if interface == WlOutput::interface().name => {
                let output: WlOutput = registry.bind(name, version.min(OUTPUT_VERSION), qh, name);
                log::event("output_added", json!({ "global": name }));
                app.add_lock_surface(&output, qh);
                app.outputs.push(output);
            }
            wl_registry::Event::GlobalRemove { name } => {
                let Some(i) = app.outputs.iter().position(|o| o.data() == Some(&name)) else {
                    return;
                };
                let output = app.outputs.remove(i);
                log::event("output_removed", json!({ "global": name }));
                if output.version() >= 3 {
                    output.release();
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlOutput, u32> for App {
    fn event(
        _: &mut Self,
        _: &WlOutput,
        event: wl_output::Event,
        global: &u32,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_output::Event::Name { name } = event {
            log::event("output_name", json!({ "global": global, "name": name }));
        }
    }
}
