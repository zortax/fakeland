//! Triggers: actions that the compositor runs when an event happens, without a round trip to the
//! driver.

use fakeland_types::{Dur, OutputId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{Action, Roi, WindowRef};

fn default_threshold() -> f64 {
    0.001
}

fn one() -> u32 {
    1
}

/// An event that fires a trigger.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TriggerEvent {
    /// A frame of the window presents.
    Presented {
        /// The window. `None` matches any window.
        #[serde(default)]
        window: Option<WindowRef>,
    },
    /// The window commits.
    Committed {
        /// The window. `None` matches any window.
        #[serde(default)]
        window: Option<WindowRef>,
    },
    /// A presented frame changes the region.
    RoiChanged {
        /// The region.
        roi: Roi,
        /// The fraction of region pixels that must change, in `0..=1`.
        #[serde(default = "default_threshold")]
        threshold: f64,
    },
    /// The compositor sends an injected input event.
    InputSent,
    /// A matching window maps.
    WindowMapped {
        /// The matcher.
        matcher: WindowRef,
    },
    /// The app sends a mark with this name.
    AppMark {
        /// The mark name.
        name: String,
    },
    /// Every `every`-th vblank of the output.
    Vblank {
        /// The interval in vblanks.
        #[serde(default = "one")]
        every: u32,
        /// The output. `None` selects the first output.
        #[serde(default)]
        output: Option<OutputId>,
    },
}

/// Runs actions each time an event happens.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Trigger {
    /// The event.
    pub on: TriggerEvent,
    /// The actions, run in order.
    pub then: Vec<Action>,
    /// The trigger fires at most this many times.
    #[serde(default)]
    pub limit: Option<u32>,
    /// The actions run this long after the event.
    #[serde(default)]
    pub delay: Option<Dur>,
    /// A name for traces.
    #[serde(default)]
    pub label: Option<String>,
}

impl Trigger {
    /// Builds a trigger without limit or delay.
    pub fn on(event: TriggerEvent, then: impl IntoIterator<Item = Action>) -> Self {
        Self {
            on: event,
            then: then.into_iter().collect(),
            limit: None,
            delay: None,
            label: None,
        }
    }

    /// Sets the limit.
    pub fn limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Sets the delay.
    pub fn delay(mut self, delay: Dur) -> Self {
        self.delay = Some(delay);
        self
    }
}
