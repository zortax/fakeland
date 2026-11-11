//! `zwp_text_input_manager_v3`: text input of clients. The compositor itself serves as the input
//! method, so session text reaches an enabled text input without an input method client.

use smithay::reexports::wayland_protocols::wp::text_input::zv3::server::zwp_text_input_manager_v3::{
    self, ZwpTextInputManagerV3,
};
use smithay::reexports::wayland_protocols::wp::text_input::zv3::server::zwp_text_input_v3::{
    self, ZwpTextInputV3,
};
use smithay::reexports::wayland_server::backend::{ClientId, GlobalId};
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New,
};

use crate::server::State;

/// The version of the text input global.
const VERSION: u32 = 1;

/// Creates the text input global.
pub fn create_text_input_global(dh: &DisplayHandle) -> GlobalId {
    dh.create_global::<State, ZwpTextInputManagerV3, ()>(VERSION, ())
}

impl GlobalDispatch<ZwpTextInputManagerV3, ()> for State {
    fn bind(
        _state: &mut Self,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<ZwpTextInputManagerV3>,
        _data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<ZwpTextInputManagerV3, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        _manager: &ZwpTextInputManagerV3,
        request: zwp_text_input_manager_v3::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let zwp_text_input_manager_v3::Request::GetTextInput { id, .. } = request {
            let resource = data_init.init(id, ());
            state.text_inputs.add(resource);
        }
    }
}

impl Dispatch<ZwpTextInputV3, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        resource: &ZwpTextInputV3,
        request: zwp_text_input_v3::Request,
        _data: &(),
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        let Some(instance) = state.text_inputs.get_mut(resource) else {
            return;
        };
        match request {
            zwp_text_input_v3::Request::Enable => instance.pending_enable = Some(true),
            zwp_text_input_v3::Request::Disable => instance.pending_enable = Some(false),
            zwp_text_input_v3::Request::Commit => {
                instance.commits = instance.commits.wrapping_add(1);
                if let Some(on) = instance.pending_enable.take() {
                    instance.enabled = on && instance.entered.is_some();
                }
            }
            zwp_text_input_v3::Request::Destroy => state.text_inputs.remove(resource),
            _ => {}
        }
    }

    fn destroyed(state: &mut Self, _client: ClientId, resource: &ZwpTextInputV3, _data: &()) {
        state.text_inputs.remove(resource);
    }
}
