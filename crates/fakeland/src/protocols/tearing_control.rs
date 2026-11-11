//! `wp_tearing_control_manager_v1`. The compositor keeps the presentation hint of each surface in
//! the surface data. A faked display never tears, so the hint only exists.

use std::sync::Mutex;

use smithay::reexports::wayland_protocols::wp::tearing_control::v1::server::{
    wp_tearing_control_manager_v1::{self, WpTearingControlManagerV1},
    wp_tearing_control_v1::{self, PresentationHint, WpTearingControlV1},
};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource, WEnum, Weak,
};
use smithay::wayland::compositor::with_states;

use crate::server::State;

/// The tearing control global.
#[derive(Debug)]
pub struct TearingControlState;

impl TearingControlState {
    /// Creates the global.
    pub fn new(dh: &DisplayHandle) -> Self {
        dh.create_global::<State, WpTearingControlManagerV1, _>(1, ());
        Self
    }
}

/// The presentation hint of a surface, in the surface data.
#[derive(Debug, Default)]
pub struct TearingHint {
    /// The latest hint. `None` while no tearing control object exists.
    pub hint: Mutex<Option<PresentationHint>>,
}

fn set_hint(surface: &WlSurface, hint: Option<PresentationHint>) -> bool {
    with_states(surface, |states| {
        states
            .data_map
            .insert_if_missing_threadsafe(TearingHint::default);
        let data = states
            .data_map
            .get::<TearingHint>()
            .expect("inserted above");
        let mut guard = data.hint.lock().unwrap_or_else(|e| e.into_inner());
        let had = guard.is_some();
        *guard = hint;
        had
    })
}

impl GlobalDispatch<WpTearingControlManagerV1, ()> for State {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<WpTearingControlManagerV1>,
        _data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WpTearingControlManagerV1, ()> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        resource: &WpTearingControlManagerV1,
        request: wp_tearing_control_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            wp_tearing_control_manager_v1::Request::GetTearingControl { id, surface } => {
                if set_hint(&surface, Some(PresentationHint::Vsync)) {
                    resource.post_error(
                        wp_tearing_control_manager_v1::Error::TearingControlExists,
                        "the surface already has a tearing control object",
                    );
                    return;
                }
                data_init.init(id, surface.downgrade());
            }
            wp_tearing_control_manager_v1::Request::Destroy => {}
            _ => {}
        }
    }
}

impl Dispatch<WpTearingControlV1, Weak<WlSurface>> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        _resource: &WpTearingControlV1,
        request: wp_tearing_control_v1::Request,
        data: &Weak<WlSurface>,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        let Ok(surface) = data.upgrade() else {
            return;
        };
        match request {
            wp_tearing_control_v1::Request::SetPresentationHint {
                hint: WEnum::Value(hint),
            } => {
                set_hint(&surface, Some(hint));
            }
            wp_tearing_control_v1::Request::Destroy => {
                set_hint(&surface, None);
            }
            _ => {}
        }
    }
}
