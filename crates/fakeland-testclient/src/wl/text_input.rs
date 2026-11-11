//! zwp_text_input_v3 events. The client enables text input while its surface has text focus;
//! a commit string becomes one input group at the done event.

use wayland_client::{Connection, Dispatch, QueueHandle};
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_v3::{
    self, ContentHint, ContentPurpose, ZwpTextInputV3,
};

use crate::app::App;
use crate::input::{GroupKind, InputEffect};

wayland_client::delegate_noop!(App: ignore ZwpTextInputManagerV3);

impl Dispatch<ZwpTextInputV3, ()> for App {
    fn event(
        app: &mut Self,
        ti: &ZwpTextInputV3,
        event: zwp_text_input_v3::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_text_input_v3::Event::Enter { .. } => {
                ti.enable();
                ti.set_content_type(ContentHint::None, ContentPurpose::Normal);
                ti.commit();
            }
            zwp_text_input_v3::Event::Leave { .. } => {
                ti.disable();
                ti.commit();
            }
            zwp_text_input_v3::Event::CommitString { text } => {
                app.seat.commit_string = text;
            }
            zwp_text_input_v3::Event::Done { .. } => {
                if let Some(text) = app.seat.commit_string.take().filter(|t| !t.is_empty()) {
                    app.input_group(GroupKind::Text, vec![InputEffect::Text { text }], None);
                }
            }
            _ => {}
        }
    }
}
