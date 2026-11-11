//! Scenario types: anchors, actions, input programs, triggers, waits and probe definitions. Rust
//! code, the control socket and the MCP server share these types.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod control;
mod expand;
mod input;
mod motion;
pub mod prelude;
mod probe;
mod scenario;
mod target;
mod trigger;
mod wait;

pub use control::{CaptureTarget, ClipboardAction, OutputAction, Transform, WindowAction};
pub use expand::{ExpandError, InputProgram, Primitive, TimedPrimitive, expand, ticks};
pub use fakeland_types::{Dur, DurParseError};
pub use input::{
    Button, Combo, ComboError, Drag, Fling, Gesture, Key, KeyAction, Modifier, PointerAction,
    PointerGlide, ScrollAxis, ScrollGlide, ScrollSource, TextVia, TouchAction, TypingCadence,
    keysym_for_char,
};
pub use motion::{Easing, MotionError, Path, PathSampler, VelocityCurve};
pub use probe::{Detector, PhaseSpec, ProbeSpec};
pub use scenario::{Action, Anchor, Scenario, ScenarioBuilder, ScenarioError, Step};
pub use target::{Position, Roi, WindowRef};
pub use trigger::{Trigger, TriggerEvent};
pub use wait::{PixelCondition, Rgba, Wait};
