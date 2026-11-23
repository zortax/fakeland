//! `zwlr_layer_shell_v1` version 4. Layer surfaces go to the layer registry, popups of layer
//! surfaces stay inside their output, and acked configures land in the trace. The xdg shell
//! tracks the popups themselves.

use smithay::delegate_layer_shell;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::shell::wlr_layer::{
    Layer, LayerSurface, LayerSurfaceConfigure, WlrLayerShellHandler, WlrLayerShellState,
};
use smithay::wayland::shell::xdg::PopupSurface;

use crate::server::State;

impl WlrLayerShellHandler for State {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.protocols.layer_shell
    }

    fn new_layer_surface(
        &mut self,
        surface: LayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        self.layer_created(surface, output, namespace);
    }

    fn new_popup(&mut self, _parent: LayerSurface, popup: PopupSurface) {
        self.unconstrain_popup(&popup);
    }

    fn ack_configure(&mut self, surface: WlSurface, configure: LayerSurfaceConfigure) {
        self.layer_configure_acked(&surface, configure);
    }

    fn layer_destroyed(&mut self, surface: LayerSurface) {
        self.layer_gone(&surface);
    }
}

delegate_layer_shell!(State);
