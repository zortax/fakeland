//! wl_pointer events. They accumulate until wl_pointer.frame, or until each event on pointers
//! older than version 5, and then form one input group. The client shows no cursor image.

use wayland_client::protocol::wl_pointer::{self, WlPointer};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};

use crate::app::App;
use crate::input::AxisSource;

impl Dispatch<WlPointer, ()> for App {
    fn event(
        app: &mut Self,
        pointer: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let acc = &mut app.seat.pointer_frame;
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface_x,
                surface_y,
                ..
            } => {
                pointer.set_cursor(serial, None, 0, 0);
                acc.enter(surface_x, surface_y);
            }
            wl_pointer::Event::Leave { .. } => acc.leave(),
            wl_pointer::Event::Motion {
                time,
                surface_x,
                surface_y,
            } => acc.motion(time, surface_x, surface_y),
            wl_pointer::Event::Button {
                time,
                button,
                state,
                ..
            } => {
                let pressed = matches!(state, WEnum::Value(wl_pointer::ButtonState::Pressed));
                acc.button(time, button, pressed);
            }
            wl_pointer::Event::Axis { time, axis, value } => {
                acc.axis(time, is_horizontal(axis), value);
            }
            wl_pointer::Event::AxisSource { axis_source } => {
                if let Some(source) = source_of(axis_source) {
                    acc.axis_source(source);
                }
            }
            wl_pointer::Event::AxisStop { time, axis } => acc.axis_stop(time, is_horizontal(axis)),
            wl_pointer::Event::AxisDiscrete { axis, discrete } => {
                acc.axis_discrete(is_horizontal(axis), discrete);
            }
            wl_pointer::Event::AxisValue120 { axis, value120 } => {
                acc.axis_value120(is_horizontal(axis), value120);
            }
            wl_pointer::Event::Frame => {
                app.finish_pointer_frame();
                return;
            }
            _ => return,
        }
        if pointer.version() < 5 {
            app.finish_pointer_frame();
        }
    }
}

fn is_horizontal(axis: WEnum<wl_pointer::Axis>) -> bool {
    matches!(axis, WEnum::Value(wl_pointer::Axis::HorizontalScroll))
}

fn source_of(source: WEnum<wl_pointer::AxisSource>) -> Option<AxisSource> {
    Some(match source.into_result().ok()? {
        wl_pointer::AxisSource::Wheel => AxisSource::Wheel,
        wl_pointer::AxisSource::Finger => AxisSource::Finger,
        wl_pointer::AxisSource::Continuous => AxisSource::Continuous,
        wl_pointer::AxisSource::WheelTilt => AxisSource::WheelTilt,
        _ => return None,
    })
}
