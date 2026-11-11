//! wl_registry events after startup: a new wl_output global is bound at once.

use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use crate::app::App;

impl Dispatch<WlRegistry, GlobalListContents> for App {
    fn event(
        app: &mut Self,
        registry: &WlRegistry,
        event: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
            && interface == WlOutput::interface().name
        {
            app.outputs.bind(registry, name, version, qh);
        }
    }
}
