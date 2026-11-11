//! Steps and their anchors: when each step starts.

use fakeland_types::{Dur, OutputId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Action;
use crate::Wait;

/// When a step starts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Anchor {
    /// As soon as the previous step finished.
    #[default]
    Immediately,
    /// At a time since the scenario started.
    At {
        /// The time since the start.
        at: Dur,
    },
    /// A time after the previous step finished.
    After {
        /// The delay.
        after: Dur,
    },
    /// At the next vblank of an output plus a phase.
    NextVblank {
        /// The output. `None` selects the output of the target window, else the first output.
        #[serde(default)]
        output: Option<OutputId>,
        /// The time after the vblank.
        #[serde(default)]
        phase: Dur,
    },
    /// When a wait condition holds.
    OnEvent(Wait),
}

/// An anchored action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Step {
    /// When the step starts.
    #[serde(default)]
    pub anchor: Anchor,
    /// What the step does.
    pub action: Action,
    /// A name for traces and reports.
    #[serde(default)]
    pub label: Option<String>,
}

impl Step {
    /// Builds a step that starts at once.
    pub fn new(action: impl Into<Action>) -> Self {
        Self {
            anchor: Anchor::Immediately,
            action: action.into(),
            label: None,
        }
    }

    /// Sets the anchor.
    pub fn anchor(mut self, anchor: Anchor) -> Self {
        self.anchor = anchor;
        self
    }

    /// Sets the label.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }
}
