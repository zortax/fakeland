//! Host input of the viewer: pointer and keyboard events on a window become session input in
//! the coordinates of its output. The host cursor hides over a window, since the composite
//! draws the session cursor. Keys pressed through the viewer release when the window loses
//! keyboard focus.

use fakeland_types::OutputId;
use wayland_client::protocol::wl_keyboard::{self, KeyState, KeymapFormat, WlKeyboard};
use wayland_client::protocol::wl_pointer::{self, Axis, AxisSource, ButtonState, WlPointer};
use wayland_client::protocol::wl_seat::{self, Capability, WlSeat};
use wayland_client::{Connection, Dispatch, Proxy, QueueHandle, WEnum};

use super::app::App;
use super::keymap::HostKeymap;
use crate::viewer::layout::to_output;
use crate::viewer::{AxisKind, ViewerInput};

/// The scroll data of one pointer frame.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AxisFrame {
    /// Horizontal distance.
    pub dx: f64,
    /// Vertical distance.
    pub dy: f64,
    /// High-resolution wheel steps.
    pub value120: Option<(i32, i32)>,
    /// The source.
    pub kind: Option<AxisKind>,
    /// The scroll ends.
    pub stop: bool,
    /// The frame holds scroll data.
    pub any: bool,
}

impl AxisFrame {
    /// Returns the viewer input of the frame and clears it.
    pub fn take(&mut self) -> Option<ViewerInput> {
        let f = std::mem::take(self);
        f.any.then(|| ViewerInput::Axis {
            dx: f.dx,
            dy: f.dy,
            value120: f.value120,
            kind: f.kind.unwrap_or(AxisKind::Wheel),
            stop: f.stop,
        })
    }
}

impl App {
    /// Maps a point on a surface to its output.
    fn map_point(&self, x: f64, y: f64) -> Option<(OutputId, f64, f64)> {
        let surface = self.pointer_on.as_ref()?;
        let output = *self.surfaces.get(surface)?;
        let win = self.windows.get(&output)?;
        let on_content = self
            .pointer_surface
            .as_ref()
            .is_some_and(|s| win.is_content(s));
        let (wx, wy) = if on_content {
            (
                x + f64::from(win.content_rect.x),
                y + f64::from(win.content_rect.y),
            )
        } else {
            (x, y)
        };
        let (ox, oy) = to_output(win.content_rect, win.logical, wx, wy);
        Some((output, ox, oy))
    }

    fn motion(&self, x: f64, y: f64) {
        if let Some((output, x, y)) = self.map_point(x, y) {
            self.input(output, ViewerInput::Motion { x, y });
        }
    }

    fn keysym(&self, code: u32) -> Option<u32> {
        self.keymap.as_ref().and_then(|k| k.base_keysym(code))
    }

    fn release_keys(&mut self) {
        let Some(output) = self.key_output.take() else {
            return;
        };
        for code in std::mem::take(&mut self.keys) {
            let keysym = self.keysym(code);
            self.input(
                output,
                ViewerInput::Key {
                    code,
                    keysym,
                    pressed: false,
                },
            );
        }
    }
}

impl Dispatch<WlSeat, ()> for App {
    fn event(
        app: &mut Self,
        seat: &WlSeat,
        event: wl_seat::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        let wl_seat::Event::Capabilities {
            capabilities: WEnum::Value(caps),
        } = event
        else {
            return;
        };
        if caps.contains(Capability::Pointer) && app.pointer.is_none() {
            app.pointer = Some(seat.get_pointer(qh, ()));
        }
        if caps.contains(Capability::Keyboard) && app.keyboard.is_none() {
            app.keyboard = Some(seat.get_keyboard(qh, ()));
        }
    }
}

impl Dispatch<WlPointer, ()> for App {
    fn event(
        app: &mut Self,
        pointer: &WlPointer,
        event: wl_pointer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_pointer::Event::Enter {
                serial,
                surface,
                surface_x,
                surface_y,
            } => {
                pointer.set_cursor(serial, None, 0, 0);
                app.pointer_on = Some(surface.id());
                app.pointer_surface = Some(surface);
                app.motion(surface_x, surface_y);
            }
            wl_pointer::Event::Leave { .. } => {
                app.pointer_on = None;
                app.pointer_surface = None;
            }
            wl_pointer::Event::Motion {
                surface_x,
                surface_y,
                ..
            } => app.motion(surface_x, surface_y),
            wl_pointer::Event::Button { button, state, .. } => {
                let Some(output) = app
                    .pointer_on
                    .as_ref()
                    .and_then(|s| app.surfaces.get(s))
                    .copied()
                else {
                    return;
                };
                let pressed = state == WEnum::Value(ButtonState::Pressed);
                app.input(
                    output,
                    ViewerInput::Button {
                        code: button,
                        pressed,
                    },
                );
            }
            wl_pointer::Event::Axis { axis, value, .. } => {
                app.axis.any = true;
                match axis {
                    WEnum::Value(Axis::VerticalScroll) => app.axis.dy += value,
                    WEnum::Value(Axis::HorizontalScroll) => app.axis.dx += value,
                    _ => {}
                }
            }
            wl_pointer::Event::AxisSource { axis_source } => {
                app.axis.kind = Some(match axis_source {
                    WEnum::Value(AxisSource::Finger) => AxisKind::Finger,
                    WEnum::Value(AxisSource::Continuous) => AxisKind::Continuous,
                    _ => AxisKind::Wheel,
                });
            }
            wl_pointer::Event::AxisStop { .. } => {
                app.axis.any = true;
                app.axis.stop = true;
            }
            wl_pointer::Event::AxisValue120 { axis, value120 } => {
                app.axis.any = true;
                let v = app.axis.value120.get_or_insert((0, 0));
                match axis {
                    WEnum::Value(Axis::VerticalScroll) => v.1 += value120,
                    WEnum::Value(Axis::HorizontalScroll) => v.0 += value120,
                    _ => {}
                }
            }
            wl_pointer::Event::Frame => {
                let output = app
                    .pointer_on
                    .as_ref()
                    .and_then(|s| app.surfaces.get(s))
                    .copied();
                if let (Some(input), Some(output)) = (app.axis.take(), output) {
                    app.input(output, input);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WlKeyboard, ()> for App {
    fn event(
        app: &mut Self,
        _: &WlKeyboard,
        event: wl_keyboard::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wl_keyboard::Event::Keymap { format, fd, size } => {
                app.keymap = match format {
                    WEnum::Value(KeymapFormat::XkbV1) => HostKeymap::from_fd(fd, size),
                    _ => None,
                };
            }
            wl_keyboard::Event::Enter { surface, .. } => {
                app.release_keys();
                app.key_output = app.surfaces.get(&surface.id()).copied();
            }
            wl_keyboard::Event::Leave { .. } => app.release_keys(),
            wl_keyboard::Event::Key { key, state, .. } => {
                let Some(output) = app.key_output else {
                    return;
                };
                let pressed = state == WEnum::Value(KeyState::Pressed);
                if pressed {
                    app.keys.insert(key);
                } else if !app.keys.remove(&key) {
                    return;
                }
                let keysym = app.keysym(key);
                app.input(
                    output,
                    ViewerInput::Key {
                        code: key,
                        keysym,
                        pressed,
                    },
                );
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axis_frames_collect_and_clear() {
        let mut f = AxisFrame::default();
        assert_eq!(f.take(), None);
        f.any = true;
        f.dy = 15.0;
        f.value120 = Some((0, 120));
        f.kind = Some(AxisKind::Wheel);
        assert_eq!(
            f.take(),
            Some(ViewerInput::Axis {
                dx: 0.0,
                dy: 15.0,
                value120: Some((0, 120)),
                kind: AxisKind::Wheel,
                stop: false
            })
        );
        assert_eq!(f, AxisFrame::default());
    }
}
