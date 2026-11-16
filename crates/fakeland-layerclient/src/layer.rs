//! The layer surface: creation with the requested state, configures and close.

use serde_json::json;
use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::{
    self, KeyboardInteractivity, ZwlrLayerSurfaceV1,
};

use crate::app::App;
use crate::args::{KeyboardArg, LayerArg};
use crate::log;

impl App {
    /// Creates the layer surface and makes its initial commit. Returns false without the layer
    /// shell.
    pub fn create_layer(&mut self, qh: &QueueHandle<Self>) -> bool {
        let Some(shell) = &self.layer_shell else {
            return false;
        };
        let a = &self.args;
        let surface = self.compositor.create_surface(qh, ());
        let output = a.output.and_then(|i| self.outputs.get(i));
        let layer = match a.layer {
            LayerArg::Background => Layer::Background,
            LayerArg::Bottom => Layer::Bottom,
            LayerArg::Top => Layer::Top,
            LayerArg::Overlay => Layer::Overlay,
        };
        let ls = shell.get_layer_surface(&surface, output, layer, a.namespace.clone(), qh, ());
        ls.set_size(a.size.0, a.size.1);
        ls.set_anchor(anchor(&a.anchor));
        let [top, right, bottom, left] = a.margin;
        ls.set_margin(top, right, bottom, left);
        ls.set_exclusive_zone(a.exclusive_zone);
        ls.set_keyboard_interactivity(match a.keyboard {
            KeyboardArg::None => KeyboardInteractivity::None,
            KeyboardArg::Exclusive => KeyboardInteractivity::Exclusive,
            KeyboardArg::OnDemand => KeyboardInteractivity::OnDemand,
        });
        self.apply_input_region(&surface, qh);
        surface.commit();
        self.layer = Some((surface, ls));
        true
    }
}

fn anchor(edges: &[String]) -> zwlr_layer_surface_v1::Anchor {
    use zwlr_layer_surface_v1::Anchor as A;
    edges.iter().fold(A::empty(), |acc, e| {
        acc | match e.trim() {
            "top" => A::Top,
            "bottom" => A::Bottom,
            "left" => A::Left,
            "right" => A::Right,
            _ => A::empty(),
        }
    })
}

impl Dispatch<ZwlrLayerSurfaceV1, ()> for App {
    fn event(
        app: &mut Self,
        ls: &ZwlrLayerSurfaceV1,
        event: zwlr_layer_surface_v1::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        match event {
            zwlr_layer_surface_v1::Event::Configure {
                serial,
                width,
                height,
            } => {
                ls.ack_configure(serial);
                log::event("configure", json!({ "width": width, "height": height }));
                let w = if width == 0 { app.args.size.0 } else { width };
                let h = if height == 0 { app.args.size.1 } else { height };
                let first = app.popup.is_none();
                if let Some((surface, _)) = &app.layer
                    && let Err(e) = app.draw(surface, w.max(1), h.max(1), app.args.color, qh)
                {
                    eprintln!("fakeland-layerclient: {e}");
                }
                if first && let Some((_, ls)) = &app.layer {
                    let ls = ls.clone();
                    app.create_popup(None, qh, |popup| ls.get_popup(popup));
                }
            }
            zwlr_layer_surface_v1::Event::Closed => {
                log::event("closed", json!({}));
                app.done = true;
            }
            _ => {}
        }
    }
}
