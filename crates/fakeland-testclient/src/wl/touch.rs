//! wl_touch events. The events up to wl_touch.frame form one input group; a cancel forms a
//! group of its own.

use fakeland_types::Point;
use wayland_client::protocol::wl_touch::{self, WlTouch};
use wayland_client::{Connection, Dispatch, QueueHandle};

use crate::app::App;
use crate::input::{GroupKind, InputEffect};

impl Dispatch<WlTouch, ()> for App {
    fn event(
        app: &mut Self,
        _: &WlTouch,
        event: wl_touch::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        let s = &mut app.seat;
        match event {
            wl_touch::Event::Down { time, id, x, y, .. } => {
                s.touch_time = Some(time);
                s.touch_effects.push(InputEffect::TouchDown {
                    id,
                    pos: Point::new(x, y),
                });
            }
            wl_touch::Event::Motion { time, id, x, y } => {
                s.touch_time = Some(time);
                s.touch_effects.push(InputEffect::TouchMotion {
                    id,
                    pos: Point::new(x, y),
                });
            }
            wl_touch::Event::Up { time, id, .. } => {
                s.touch_time = Some(time);
                s.touch_effects.push(InputEffect::TouchUp { id });
            }
            wl_touch::Event::Frame => {
                let effects = std::mem::take(&mut s.touch_effects);
                let time = s.touch_time.take();
                if !effects.is_empty() {
                    app.input_group(GroupKind::Touch, effects, time);
                }
            }
            wl_touch::Event::Cancel => {
                s.touch_effects.clear();
                s.touch_time = None;
                app.input_group(GroupKind::Touch, vec![InputEffect::TouchCancel], None);
            }
            _ => {}
        }
    }
}
