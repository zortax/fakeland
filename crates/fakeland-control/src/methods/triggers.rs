//! Trigger sets: declarative triggers that stay armed on the compositor thread until removed.
//! A set is a run with one long sleep step and the triggers.

use fakeland::TriggerFiring;
use fakeland_scenario::Trigger;
use fakeland_types::{Dur, RunId};
use serde::{Deserialize, Serialize};

use super::defaults;
use super::method::method;

/// Triggers to install.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InstallTriggersParams {
    /// The triggers.
    pub triggers: Vec<Trigger>,
    /// The set ends after this time.
    #[serde(default = "defaults::trigger_lifetime")]
    pub lifetime: Dur,
    /// A name for the set.
    #[serde(default)]
    pub name: Option<String>,
}

/// An installed trigger set.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TriggerSet {
    /// The id of the run that holds the set.
    pub set: RunId,
}

/// A trigger set to remove.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoveTriggersParams {
    /// The set.
    pub set: RunId,
}

method!(
    /// Arms a trigger set.
    InstallTriggers, "install_triggers", InstallTriggersParams => TriggerSet
);
method!(
    /// Disarms a trigger set and returns every firing.
    RemoveTriggers, "remove_triggers", RemoveTriggersParams => Vec<TriggerFiring>
);
