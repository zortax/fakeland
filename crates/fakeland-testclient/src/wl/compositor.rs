//! wl_compositor, wl_surface and frame callback events: entered outputs and the preferred
//! buffer scale feed the scale state, and wl_callback.done ends the frame callback wait.

use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_surface::{self, WlSurface};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle};

use crate::app::App;

/// The user data of a frame callback: the frame whose commit requested it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameTag(pub u64);

wayland_client::delegate_noop!(App: ignore WlCompositor);

impl Dispatch<WlSurface, ()> for App {
    fn event(
        app: &mut Self,
        _: &WlSurface,
        event: wl_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_surface::Event::Enter { output } => {
                app.outputs.enter(output.id());
                let scale = app.outputs.entered_scale();
                app.scale.set_outputs(scale);
                app.scale_hint_changed();
            }
            wl_surface::Event::Leave { output } => {
                app.outputs.leave(&output.id());
                let scale = app.outputs.entered_scale();
                app.scale.set_outputs(scale);
                app.scale_hint_changed();
            }
            wl_surface::Event::PreferredBufferScale { factor } => {
                app.scale.set_preferred(factor);
                app.scale_hint_changed();
            }
            _ => {}
        }
    }
}

impl Dispatch<WlCallback, FrameTag> for App {
    fn event(
        app: &mut Self,
        _: &WlCallback,
        event: wl_callback::Event,
        tag: &FrameTag,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_callback::Event::Done { callback_data } = event {
            app.frame_done(tag.0, callback_data);
        }
    }
}
