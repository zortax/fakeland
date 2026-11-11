//! Input methods: raw primitives, single actions and convenience input. Each call returns the
//! events that reached clients, with compositor send times.

use fakeland::{ProbeReport, SentInput, WaitOutput};
use fakeland_scenario::{
    Action, Button, Key, Position, Primitive, ScrollSource, TextVia, TypingCadence,
};
use fakeland_types::Dur;
use serde::{Deserialize, Serialize};

use super::ShotInfo;
use super::defaults;
use super::method::method;

/// Primitives to send at once.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InjectParams {
    /// The primitives in send order.
    pub primitives: Vec<Primitive>,
}

/// One scenario action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActParams {
    /// The action.
    pub action: Action,
}

/// The result of one action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ActOutcome {
    /// The action finished without a value.
    Done,
    /// The input events that reached clients.
    Input {
        /// The events in send order.
        sent: Vec<SentInput>,
    },
    /// A screenshot with its PNG inline.
    Captured {
        /// The label of the capture action.
        label: Option<String>,
        /// The screenshot.
        shot: Box<ShotInfo>,
    },
    /// A probe report.
    Probe {
        /// The report.
        report: Box<ProbeReport>,
    },
    /// The result of a wait.
    Waited {
        /// The wait output.
        output: Box<WaitOutput>,
    },
}

/// A pointer motion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PointerMoveParams {
    /// The target position.
    pub pos: Position,
}

/// One or more clicks.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClickParams {
    /// Moves here first. `None` clicks at the pointer.
    #[serde(default)]
    pub pos: Option<Position>,
    /// The button.
    #[serde(default)]
    pub button: Button,
    /// The count of clicks: 2 is a double click.
    #[serde(default = "defaults::one")]
    pub count: u32,
}

/// A scroll.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScrollParams {
    /// Moves here first. `None` scrolls at the pointer.
    #[serde(default)]
    pub at: Option<Position>,
    /// Horizontal distance in logical pixels. Positive scrolls right.
    #[serde(default)]
    pub dx: f64,
    /// Vertical distance in logical pixels. Positive scrolls down.
    #[serde(default)]
    pub dy: f64,
    /// The device.
    #[serde(default)]
    pub source: ScrollSource,
    /// Spreads the distance over this time at a constant velocity. `None` sends one event.
    #[serde(default)]
    pub smooth: Option<Dur>,
}

/// A drag with a button held.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DragParams {
    /// The start position.
    pub from: Position,
    /// The end position.
    pub to: Position,
    /// The drag time.
    #[serde(default = "defaults::drag_duration")]
    pub duration: Dur,
    /// The held button.
    #[serde(default)]
    pub button: Button,
}

/// A key tap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct KeyParams {
    /// The key: an xkb keysym name or an evdev code.
    pub key: Key,
}

/// A key combination.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComboParams {
    /// The combination, such as `ctrl+shift+t`.
    pub combo: String,
}

/// Typed text.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TypeTextParams {
    /// The text.
    pub text: String,
    /// The typing cadence.
    #[serde(default)]
    pub cadence: TypingCadence,
    /// Key events or one text-input commit.
    #[serde(default)]
    pub via: TextVia,
}

/// A touch tap.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TouchTapParams {
    /// The position.
    pub pos: Position,
}

method!(
    /// Sends primitives at once.
    Inject, "inject", InjectParams => Vec<SentInput>
);
method!(
    /// Runs one scenario action and returns its outcome.
    Act, "act", ActParams => ActOutcome
);
method!(
    /// Moves the pointer.
    PointerMove, "pointer_move", PointerMoveParams => Vec<SentInput>
);
method!(
    /// Clicks a button.
    Click, "click", ClickParams => Vec<SentInput>
);
method!(
    /// Scrolls by a distance.
    Scroll, "scroll", ScrollParams => Vec<SentInput>
);
method!(
    /// Drags along a line.
    DragTo, "drag", DragParams => Vec<SentInput>
);
method!(
    /// Taps a key.
    KeyTap, "key", KeyParams => Vec<SentInput>
);
method!(
    /// Presses a key combination.
    KeyCombo, "combo", ComboParams => Vec<SentInput>
);
method!(
    /// Types text.
    TypeText, "type_text", TypeTextParams => Vec<SentInput>
);
method!(
    /// Taps a touch point.
    TouchTap, "touch_tap", TouchTapParams => Vec<SentInput>
);
