//! Keyboard actions: presses, releases, taps and combinations.

use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{Combo, Key};

fn tap_hold() -> Dur {
    Dur::from_millis(30)
}

fn combo_gap() -> Dur {
    Dur::from_millis(10)
}

/// A keyboard action.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum KeyAction {
    /// Presses a key.
    Press {
        /// The key.
        key: Key,
    },
    /// Releases a key.
    Release {
        /// The key.
        key: Key,
    },
    /// Presses a key and releases it after `hold`.
    Tap {
        /// The key.
        key: Key,
        /// The time the key stays down.
        #[serde(default = "tap_hold")]
        hold: Dur,
    },
    /// Presses the modifiers, taps the key and releases the modifiers in reverse order.
    Combo {
        /// The combination, for example `"ctrl+shift+t"`.
        combo: Combo,
        /// The time the key stays down.
        #[serde(default = "tap_hold")]
        hold: Dur,
        /// The time between consecutive presses and between consecutive releases.
        #[serde(default = "combo_gap")]
        gap: Dur,
    },
}

impl KeyAction {
    /// Taps a key.
    pub fn tap(key: impl Into<Key>) -> Self {
        Self::Tap {
            key: key.into(),
            hold: tap_hold(),
        }
    }

    /// Runs a combination.
    pub fn combo(combo: Combo) -> Self {
        Self::Combo {
            combo,
            hold: tap_hold(),
            gap: combo_gap(),
        }
    }
}
