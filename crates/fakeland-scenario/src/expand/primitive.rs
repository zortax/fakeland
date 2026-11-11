//! Primitive input events and the time-ordered program they form.

use fakeland_types::{Logical, Nanos, Size};
use serde::{Deserialize, Serialize};

use crate::{Button, Key, Position, ScrollSource, WindowRef};

/// One event that the compositor sends or one configure it issues.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Primitive {
    /// Absolute pointer motion.
    PointerMotion {
        /// The target.
        pos: Position,
    },
    /// A button press or release.
    PointerButton {
        /// The button.
        button: Button,
        /// True presses.
        pressed: bool,
    },
    /// An axis event, or an axis stop when `stop` is set.
    PointerAxis {
        /// Horizontal distance in pixels.
        dx: f64,
        /// Vertical distance in pixels.
        dy: f64,
        /// The axis source.
        source: ScrollSource,
        /// High-resolution wheel steps `(x, y)`; 120 is one detent.
        value120: Option<(i32, i32)>,
        /// The event ends the scroll sequence (`axis_stop`).
        stop: bool,
    },
    /// Ends a group of pointer events (`wl_pointer.frame`).
    PointerFrame,
    /// A key press or release.
    Key {
        /// The key.
        key: Key,
        /// True presses.
        pressed: bool,
    },
    /// A text-input commit.
    Text {
        /// The text.
        text: String,
    },
    /// A touch point goes down.
    TouchDown {
        /// The touch point.
        id: u32,
        /// The position.
        pos: Position,
    },
    /// A touch point moves.
    TouchMotion {
        /// The touch point.
        id: u32,
        /// The position.
        pos: Position,
    },
    /// A touch point lifts.
    TouchUp {
        /// The touch point.
        id: u32,
    },
    /// Ends a group of touch events.
    TouchFrame,
    /// A swipe begins.
    SwipeBegin {
        /// The finger count.
        fingers: u32,
    },
    /// A swipe moves by a delta.
    SwipeUpdate {
        /// Horizontal delta.
        dx: f64,
        /// Vertical delta.
        dy: f64,
    },
    /// A swipe ends.
    SwipeEnd {
        /// The swipe ends cancelled.
        cancelled: bool,
    },
    /// A pinch begins.
    PinchBegin {
        /// The finger count.
        fingers: u32,
    },
    /// A pinch changes.
    PinchUpdate {
        /// Horizontal delta of the center.
        dx: f64,
        /// Vertical delta of the center.
        dy: f64,
        /// The scale relative to the begin.
        scale: f64,
        /// The rotation delta in degrees, clockwise.
        rotation: f64,
    },
    /// A pinch ends.
    PinchEnd {
        /// The pinch ends cancelled.
        cancelled: bool,
    },
    /// A hold begins.
    HoldBegin {
        /// The finger count.
        fingers: u32,
    },
    /// A hold ends.
    HoldEnd {
        /// The hold ends cancelled.
        cancelled: bool,
    },
    /// A configure with a new window size.
    WindowResize {
        /// The window.
        window: WindowRef,
        /// The logical size.
        size: Size<Logical>,
    },
}

/// A primitive at an offset from the program start.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimedPrimitive {
    /// The offset from the program start.
    pub offset: Nanos,
    /// The event.
    pub primitive: Primitive,
}

/// Time-ordered primitives. Events at the same offset keep their order.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputProgram {
    /// The events, sorted by offset.
    pub events: Vec<TimedPrimitive>,
}

impl InputProgram {
    /// Builds an empty program.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an event. Call `sort` when events arrive out of order.
    pub fn push(&mut self, offset: Nanos, primitive: Primitive) {
        self.events.push(TimedPrimitive { offset, primitive });
    }

    /// Sorts the events by offset and keeps the order of equal offsets.
    pub fn sort(&mut self) {
        self.events.sort_by_key(|e| e.offset);
    }

    /// Returns the offset of the last event.
    pub fn duration(&self) -> Nanos {
        self.events.last().map_or(Nanos::ZERO, |e| e.offset)
    }

    /// Returns the number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Returns true for a program without events.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Returns the primitives in order.
    pub fn primitives(&self) -> impl Iterator<Item = &Primitive> {
        self.events.iter().map(|e| &e.primitive)
    }
}
