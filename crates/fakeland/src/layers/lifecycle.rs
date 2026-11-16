//! Layer surface lifecycle: creation on an output, configures on commit, mapping when the first
//! buffer arrives, unmapping when the buffer goes, close on output removal and destruction.

use fakeland_trace::Event;
use fakeland_types::{OutputId, WindowId};
use smithay::backend::renderer::utils::with_renderer_surface_state;
use smithay::desktop::LayerSurface;
use smithay::output::Output;
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::shell::wlr_layer::{
    KeyboardInteractivity, Layer, LayerSurface as WlrLayerSurface, LayerSurfaceConfigure,
};

use super::LayerRecord;
use crate::server::{ClientState, State};

impl State {
    /// Registers a new layer surface on its output. The output is the one the client names, else
    /// the launch output of its process, else the primary output. The surface joins the layer map
    /// of the output at its initial commit and leaves it when it unmaps.
    pub fn layer_created(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        namespace: String,
    ) {
        let wl = surface.wl_surface().clone();
        let Some(sid) = self.surfaces.id_of(&wl) else {
            return;
        };
        let pid = wl
            .client()
            .and_then(|c| c.get_data::<ClientState>().and_then(|d| d.pid));
        let output = output
            .as_ref()
            .and_then(Output::from_resource)
            .and_then(|o| self.outputs.id_of(&o))
            .unwrap_or_else(|| self.output_for_new_window(pid));
        let layer = LayerSurface::new(surface, namespace);
        if self.outputs.get(output).is_none() {
            layer.layer_surface().send_close();
            return;
        }
        let id = self.ids.window();
        self.layers.insert(LayerRecord {
            id,
            layer,
            surface: sid,
            output,
            mapped_at: None,
            configured: false,
            pid,
            app: pid.and_then(|p| self.processes.root_of(p)),
        });
        self.surface_role(&wl, "zwlr_layer_surface_v1", None);
    }

    /// Runs the shell logic of an applied commit of a layer surface root. Returns false when the
    /// surface is no layer surface.
    pub fn layer_committed(&mut self, surface: &WlSurface) -> bool {
        let Some(id) = self
            .surfaces
            .id_of(surface)
            .and_then(|s| self.layers.by_surface(s))
        else {
            return false;
        };
        let Some(rec) = self.layers.get(id) else {
            return true;
        };
        let (layer, output) = (rec.layer.clone(), rec.output);
        let (mapped, configured) = (rec.mapped_at.is_some(), rec.configured);
        self.arrange_layers(output);
        let has_buffer =
            with_renderer_surface_state(surface, |s| s.buffer().is_some()).unwrap_or(false);
        match (has_buffer, mapped) {
            (false, true) => self.unmap_layer(id),
            (false, false) if !configured => {
                if let Some(rec) = self.layers.get_mut(id) {
                    rec.configured = true;
                }
                self.change_layer_map(output, |map| {
                    if let Err(e) = map.map_layer(&layer) {
                        tracing::debug!("layer surface not mapped: {e}");
                    }
                });
                self.send_layer_configure(&layer);
            }
            (true, false) => self.map_layer(id),
            _ => {}
        }
        self.refresh_keyboard_focus();
        self.space_dirty = true;
        true
    }

    /// Sends a configure with the arranged size and records it.
    fn send_layer_configure(&mut self, layer: &LayerSurface) {
        let surface = layer.layer_surface();
        let serial = surface.send_configure();
        let size = surface.with_pending_state(|s| s.size).unwrap_or_default();
        let Some(sid) = self.surfaces.id_of(surface.wl_surface()) else {
            return;
        };
        self.emit(Event::Configure {
            surface: sid,
            serial: serial.into(),
            width: size.w,
            height: size.h,
            states: Vec::new(),
        });
    }

    /// Records an acked layer surface configure. The serial goes on the next commit.
    pub fn layer_configure_acked(&mut self, surface: &WlSurface, configure: LayerSurfaceConfigure) {
        let serial: u32 = configure.serial.into();
        let Some(rec) = self.surfaces.record_mut(surface) else {
            return;
        };
        rec.acked_serial = Some(serial);
        let id = rec.id;
        self.emit(Event::AckConfigure {
            surface: id,
            serial,
        });
    }

    /// Maps a layer surface. A surface on the top or overlay layer that takes keyboard focus
    /// gets it now.
    fn map_layer(&mut self, id: WindowId) {
        let now = self.now();
        let Some(rec) = self.layers.get_mut(id) else {
            return;
        };
        rec.mapped_at = Some(now);
        let (surface, output) = (rec.surface, rec.output);
        let layer = rec.layer.clone();
        let wl = layer.wl_surface().clone();
        let state = layer.cached_state();
        let raised = matches!(state.layer, Layer::Top | Layer::Overlay);
        let takes_focus = !matches!(state.keyboard_interactivity, KeyboardInteractivity::None);
        if raised && takes_focus {
            self.layers.focus = Some(id);
        }
        self.emit(Event::WindowMapped {
            window: id,
            surface,
            app_id: layer.namespace().to_owned(),
            title: String::new(),
            output: Some(output),
        });
        self.send_tree_scale(&wl);
    }

    /// Unmaps a layer surface. Keyboard focus that it took on demand goes back.
    fn unmap_layer(&mut self, id: WindowId) {
        let Some(rec) = self.layers.get_mut(id) else {
            return;
        };
        if rec.mapped_at.take().is_none() {
            return;
        }
        rec.configured = false;
        let (layer, output) = (rec.layer.clone(), rec.output);
        if self.layers.focus == Some(id) {
            self.layers.focus = None;
        }
        self.change_layer_map(output, |map| map.unmap_layer(&layer));
        self.space_dirty = true;
        self.emit(Event::WindowUnmapped { window: id });
    }

    /// Handles the destruction of a layer surface.
    pub fn layer_gone(&mut self, surface: &WlrLayerSurface) {
        let Some(id) = self
            .layers
            .iter()
            .find(|l| l.layer.layer_surface() == surface)
            .map(|l| l.id)
        else {
            return;
        };
        self.forget_layer(id);
    }

    /// Unmaps and drops a layer surface, and arranges its output again.
    pub fn forget_layer(&mut self, id: WindowId) {
        self.unmap_layer(id);
        let Some(rec) = self.layers.remove(id) else {
            return;
        };
        self.change_layer_map(rec.output, |map| map.unmap_layer(&rec.layer));
        self.refresh_keyboard_focus();
        self.space_dirty = true;
    }

    /// Closes every layer surface of an output that goes away.
    pub fn close_output_layers(&mut self, output: OutputId) {
        for id in self.layers.on_output(output) {
            if let Some(rec) = self.layers.get(id) {
                rec.layer.layer_surface().send_close();
            }
            self.forget_layer(id);
        }
    }
}
