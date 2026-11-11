//! The configuration of the input devices on `seat0`: the xkb keymap, key repeat, the pointer
//! device model and touch.

use fakeland_types::Hz;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// An xkb keymap given by rules, model, layout, variant and options.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Keymap {
    /// The xkb rules. Empty selects the default rules.
    pub rules: String,
    /// The keyboard model. Empty selects the default model.
    pub model: String,
    /// The layout list, for example `us` or `us,de`.
    pub layout: String,
    /// The layout variants.
    pub variant: String,
    /// The xkb options, for example `ctrl:nocaps`.
    pub options: Option<String>,
}

impl Keymap {
    /// Builds a keymap with one layout.
    pub fn layout(layout: impl Into<String>) -> Self {
        Self {
            layout: layout.into(),
            ..Self::default()
        }
    }
}

/// Key repeat settings that the keyboard advertises.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct Repeat {
    /// Delay before the first repeat, in milliseconds.
    pub delay_ms: i32,
    /// Repeats per second. Zero disables repeat.
    pub rate: i32,
}

impl Default for Repeat {
    fn default() -> Self {
        Self {
            delay_ms: 600,
            rate: 25,
        }
    }
}

/// The wheel of the pointer device.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WheelKind {
    /// A high-resolution wheel: fractions of a detent reach the client as `value120`.
    #[default]
    HiRes,
    /// A classic wheel: only whole detents reach the client. Fractions add up until they make a
    /// detent.
    Discrete,
}

/// The pointer device model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PointerDevice {
    /// The motion report rate. Motion events closer than one report period merge into the next
    /// report, and the latest position wins. `None` sends every motion at once.
    pub report_rate: Option<Hz>,
    /// The wheel.
    pub wheel: WheelKind,
}

/// How keys reach a client when the keymap lacks their keysym.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UnmappedKeys {
    /// Through text input when the focused client enabled it, else through a temporary keymap.
    #[default]
    TextInput,
    /// Always through a temporary keymap, so the client sees key events.
    Keymap,
}

/// The input devices of the seat.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct InputConfig {
    /// The keymap.
    pub keymap: Keymap,
    /// Key repeat.
    pub repeat: Repeat,
    /// The pointer device.
    pub pointer: PointerDevice,
    /// How keys outside the keymap reach a client.
    pub unmapped: UnmappedKeys,
    /// The seat has a touch device.
    pub touch: bool,
}
