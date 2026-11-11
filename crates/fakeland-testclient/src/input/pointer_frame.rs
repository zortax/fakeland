//! Accumulation of wl_pointer events up to wl_pointer.frame into one input group. Axis events of
//! a frame merge into one `AxisFrame`, which follows the other effects of the frame.

use fakeland_types::{Logical, Point};

use super::{AxisFrame, AxisSource, GroupKind, InputEffect};

/// One finished pointer frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PointerFrame {
    /// The kind of the frame.
    pub kind: GroupKind,
    /// The effects in protocol order, with the merged axis effect last.
    pub effects: Vec<InputEffect>,
    /// The latest event time of the frame in milliseconds.
    pub wl_time: Option<u32>,
}

/// The pointer state and the events of the open frame.
#[derive(Debug, Default)]
pub struct PointerAccumulator {
    pos: Option<Point<Logical>>,
    effects: Vec<InputEffect>,
    axis: AxisFrame,
    has_button: bool,
    time: Option<u32>,
}

impl PointerAccumulator {
    /// Records wl_pointer.enter.
    pub fn enter(&mut self, x: f64, y: f64) {
        let pos = Point::new(x, y);
        self.pos = Some(pos);
        self.effects.push(InputEffect::PointerEnter { pos });
    }

    /// Records wl_pointer.leave.
    pub fn leave(&mut self) {
        self.pos = None;
        self.effects.push(InputEffect::PointerLeave);
    }

    /// Records wl_pointer.motion.
    pub fn motion(&mut self, time: u32, x: f64, y: f64) {
        let pos = Point::new(x, y);
        self.pos = Some(pos);
        self.time = Some(time);
        self.effects.push(InputEffect::PointerMotion { pos });
    }

    /// Records wl_pointer.button.
    pub fn button(&mut self, time: u32, button: u32, pressed: bool) {
        self.time = Some(time);
        self.has_button = true;
        self.effects.push(InputEffect::Button {
            button,
            pressed,
            pos: self.pos,
        });
    }

    /// Records wl_pointer.axis.
    pub fn axis(&mut self, time: u32, horizontal: bool, value: f64) {
        self.time = Some(time);
        if horizontal {
            self.axis.dx += value;
        } else {
            self.axis.dy += value;
        }
    }

    /// Records wl_pointer.axis_source.
    pub fn axis_source(&mut self, source: AxisSource) {
        self.axis.source = Some(source);
    }

    /// Records wl_pointer.axis_stop.
    pub fn axis_stop(&mut self, time: u32, horizontal: bool) {
        self.time = Some(time);
        if horizontal {
            self.axis.stop_x = true;
        } else {
            self.axis.stop_y = true;
        }
    }

    /// Records wl_pointer.axis_discrete.
    pub fn axis_discrete(&mut self, horizontal: bool, steps: i32) {
        if horizontal {
            self.axis.discrete_x += steps;
        } else {
            self.axis.discrete_y += steps;
        }
    }

    /// Records wl_pointer.axis_value120.
    pub fn axis_value120(&mut self, horizontal: bool, value120: i32) {
        if horizontal {
            self.axis.value120_x += value120;
        } else {
            self.axis.value120_y += value120;
        }
    }

    /// Closes the frame. Returns `None` for a frame without events.
    pub fn frame(&mut self) -> Option<PointerFrame> {
        let axis = std::mem::take(&mut self.axis);
        let mut effects = std::mem::take(&mut self.effects);
        let has_button = std::mem::take(&mut self.has_button);
        let wl_time = self.time.take();
        if !axis.is_empty() {
            effects.push(InputEffect::Axis(axis));
        }
        if effects.is_empty() {
            return None;
        }
        Some(PointerFrame {
            kind: GroupKind::of_pointer_frame(has_button, &axis),
            effects,
            wl_time,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_and_motion_form_one_motion_frame() {
        let mut p = PointerAccumulator::default();
        p.enter(1.0, 2.0);
        p.motion(100, 3.0, 4.0);
        let f = p.frame().unwrap();
        assert_eq!(f.kind, GroupKind::PointerMotion);
        assert_eq!(f.wl_time, Some(100));
        assert_eq!(f.effects.len(), 2);
        assert!(p.frame().is_none());
    }

    #[test]
    fn axis_events_merge() {
        let mut p = PointerAccumulator::default();
        p.axis_source(AxisSource::Wheel);
        p.axis_value120(false, 60);
        p.axis(5, false, 7.5);
        p.axis(5, true, -2.0);
        let f = p.frame().unwrap();
        assert_eq!(f.kind, GroupKind::PointerAxis);
        let InputEffect::Axis(a) = &f.effects[0] else {
            panic!("axis effect")
        };
        assert_eq!(a.source, Some(AxisSource::Wheel));
        assert_eq!(a.value120_y, 60);
        assert_eq!(a.dy, 7.5);
        assert_eq!(a.dx, -2.0);
    }

    #[test]
    fn stop_alone_is_an_axis_frame() {
        let mut p = PointerAccumulator::default();
        p.axis_source(AxisSource::Finger);
        p.axis_stop(9, false);
        let f = p.frame().unwrap();
        assert_eq!(f.kind, GroupKind::PointerAxis);
        let InputEffect::Axis(a) = &f.effects[0] else {
            panic!("axis effect")
        };
        assert!(a.stop_y);
    }

    #[test]
    fn button_carries_the_position() {
        let mut p = PointerAccumulator::default();
        p.enter(10.0, 20.0);
        p.frame();
        p.button(7, 0x110, true);
        let f = p.frame().unwrap();
        assert_eq!(f.kind, GroupKind::PointerButton);
        assert_eq!(
            f.effects,
            vec![InputEffect::Button {
                button: 0x110,
                pressed: true,
                pos: Some(Point::new(10.0, 20.0)),
            }]
        );
    }
}
