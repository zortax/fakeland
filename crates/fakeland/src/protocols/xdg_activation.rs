//! `xdg_activation_v1`. A valid token focuses the window of the requesting surface.

use smithay::delegate_xdg_activation;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::xdg_activation::{
    XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
};

use crate::server::State;

/// Tokens older than this are stale.
const TOKEN_LIFETIME_SECS: u64 = 10;

impl XdgActivationHandler for State {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.protocols.xdg_activation
    }

    fn request_activation(
        &mut self,
        _token: XdgActivationToken,
        token_data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        if token_data.timestamp.elapsed().as_secs() >= TOKEN_LIFETIME_SECS {
            return;
        }
        if let Some(window) = self.window_of_surface(&surface) {
            self.focus_window(Some(window));
        }
    }
}

delegate_xdg_activation!(State);
