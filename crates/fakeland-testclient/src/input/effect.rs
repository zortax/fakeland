//! Decoded input events in surface-local logical coordinates. Scenes consume these effects; the
//! Wayland handlers produce them.

use fakeland_types::{Logical, Point};
use serde::Serialize;

/// The source of a scroll frame, as wl_pointer.axis_source reports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AxisSource {
    /// A scroll wheel.
    Wheel,
    /// A finger on a touchpad.
    Finger,
    /// A continuous source such as a trackpoint.
    Continuous,
    /// A tilting wheel.
    WheelTilt,
}

/// One pointer frame worth of scroll data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct AxisFrame {
    /// The scroll source, when the frame names one.
    pub source: Option<AxisSource>,
    /// Horizontal scroll in logical pixels. Positive scrolls right.
    pub dx: f64,
    /// Vertical scroll in logical pixels. Positive scrolls down.
    pub dy: f64,
    /// Horizontal high-resolution wheel steps; 120 is one detent.
    pub value120_x: i32,
    /// Vertical high-resolution wheel steps; 120 is one detent.
    pub value120_y: i32,
    /// Horizontal discrete steps from compositors older than wl_seat v8.
    pub discrete_x: i32,
    /// Vertical discrete steps from compositors older than wl_seat v8.
    pub discrete_y: i32,
    /// The horizontal axis stopped.
    pub stop_x: bool,
    /// The vertical axis stopped.
    pub stop_y: bool,
}

impl AxisFrame {
    /// Returns true when the frame holds no axis data at all.
    pub fn is_empty(&self) -> bool {
        self.source.is_none() && *self == Self::default()
    }
}

/// A decoded input event.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEffect {
    /// The pointer entered the surface.
    PointerEnter {
        /// Surface-local position.
        pos: Point<Logical>,
    },
    /// The pointer moved on the surface.
    PointerMotion {
        /// Surface-local position.
        pos: Point<Logical>,
    },
    /// The pointer left the surface.
    PointerLeave,
    /// A pointer button changed state.
    Button {
        /// The evdev button code.
        button: u32,
        /// True for a press.
        pressed: bool,
        /// The pointer position, when the pointer is on the surface.
        pos: Option<Point<Logical>>,
    },
    /// A scroll frame.
    Axis(AxisFrame),
    /// A key changed state.
    Key {
        /// The evdev key code.
        code: u32,
        /// The xkb keysym.
        keysym: u32,
        /// The text the key produces, possibly empty.
        utf8: String,
        /// True for a press.
        pressed: bool,
    },
    /// Text from an input method.
    Text {
        /// The committed text.
        text: String,
    },
    /// A touch point went down.
    TouchDown {
        /// The touch point id.
        id: i32,
        /// Surface-local position.
        pos: Point<Logical>,
    },
    /// A touch point moved.
    TouchMotion {
        /// The touch point id.
        id: i32,
        /// Surface-local position.
        pos: Point<Logical>,
    },
    /// A touch point went up.
    TouchUp {
        /// The touch point id.
        id: i32,
    },
    /// The compositor cancelled every touch point.
    TouchCancel,
}
