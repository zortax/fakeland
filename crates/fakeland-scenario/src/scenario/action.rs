//! The action of a scenario step or a trigger.

use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Step;
use crate::{
    CaptureTarget, ClipboardAction, Drag, Fling, Gesture, KeyAction, OutputAction, PointerAction,
    PointerGlide, ProbeSpec, ScrollGlide, TextVia, TouchAction, TypingCadence, Wait, WindowAction,
};

/// One thing a scenario does. JSON tags it with `type`; grouped actions add an `op` tag.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// A pointer action.
    Pointer(PointerAction),
    /// A keyboard action.
    Key(KeyAction),
    /// Types text.
    Type {
        /// The text.
        text: String,
        /// The timing of the key presses.
        #[serde(default)]
        cadence: TypingCadence,
        /// How the text reaches the client.
        #[serde(default)]
        via: TextVia,
    },
    /// A touch action.
    Touch(TouchAction),
    /// Moves the pointer along a path.
    Glide(PointerGlide),
    /// Scrolls along a velocity curve.
    ScrollGlide(ScrollGlide),
    /// Drags with a held button.
    Drag(Drag),
    /// A finger scroll with decaying velocity.
    Fling(Fling),
    /// A touchpad gesture.
    Gesture(Gesture),
    /// Changes an output.
    Output(OutputAction),
    /// Manages a window.
    Window(WindowAction),
    /// Writes a named mark into the trace.
    Mark {
        /// The name.
        label: String,
    },
    /// Takes a screenshot.
    Capture {
        /// What to capture.
        #[serde(default)]
        target: CaptureTarget,
        /// A name for the result.
        #[serde(default)]
        label: Option<String>,
        /// Downscales so the longer side has at most this many pixels.
        #[serde(default)]
        max_dim: Option<u32>,
    },
    /// Runs a latency probe.
    Probe(ProbeSpec),
    /// Waits for a condition.
    Wait(Wait),
    /// Waits for a time.
    Sleep {
        /// The time.
        duration: Dur,
    },
    /// Runs steps `count` times.
    Repeat {
        /// The number of runs.
        count: u32,
        /// The steps of each run.
        steps: Vec<Step>,
    },
    /// Changes the clipboard.
    Clipboard(ClipboardAction),
}

impl Action {
    /// Returns true for actions that inject input or configure window sizes, so they expand to an
    /// input program.
    pub fn is_input(&self) -> bool {
        match self {
            Self::Pointer(_)
            | Self::Key(_)
            | Self::Type { .. }
            | Self::Touch(_)
            | Self::Glide(_)
            | Self::ScrollGlide(_)
            | Self::Drag(_)
            | Self::Fling(_)
            | Self::Gesture(_) => true,
            Self::Window(w) => matches!(
                w,
                WindowAction::Resize { .. } | WindowAction::ResizeStorm { .. }
            ),
            _ => false,
        }
    }

    /// Types text at the default cadence.
    pub fn type_text(text: impl Into<String>) -> Self {
        Self::Type {
            text: text.into(),
            cadence: TypingCadence::default(),
            via: TextVia::Keys,
        }
    }

    /// Writes a mark.
    pub fn mark(label: impl Into<String>) -> Self {
        Self::Mark {
            label: label.into(),
        }
    }

    /// Sleeps.
    pub fn sleep(duration: Dur) -> Self {
        Self::Sleep { duration }
    }

    /// Captures a target.
    pub fn capture(target: CaptureTarget) -> Self {
        Self::Capture {
            target,
            label: None,
            max_dim: None,
        }
    }
}

macro_rules! from_inner {
    ($($ty:ident => $variant:ident),* $(,)?) => {
        $(impl From<$ty> for Action {
            fn from(inner: $ty) -> Self {
                Self::$variant(inner)
            }
        })*
    };
}

from_inner!(
    PointerAction => Pointer,
    KeyAction => Key,
    TouchAction => Touch,
    PointerGlide => Glide,
    ScrollGlide => ScrollGlide,
    Drag => Drag,
    Fling => Fling,
    Gesture => Gesture,
    OutputAction => Output,
    WindowAction => Window,
    ProbeSpec => Probe,
    Wait => Wait,
    ClipboardAction => Clipboard,
);
