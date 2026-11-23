//! Output methods.

use fakeland::{OutputChange, OutputConfig, OutputState};
use fakeland_types::OutputId;
use serde::{Deserialize, Serialize};

use super::method::{NoParams, method};

/// A change to a running output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SetOutputParams {
    /// The output. `None` selects the primary output.
    #[serde(default)]
    pub output: Option<OutputId>,
    /// The change.
    pub change: OutputChange,
}

/// A new output.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AddOutputParams {
    /// The output config.
    pub config: OutputConfig,
}

/// An output to remove.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoveOutputParams {
    /// The output.
    pub output: OutputId,
}

/// An unplugged output to plug in again.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplugOutputParams {
    /// The connector name. `None` selects the latest unplugged output.
    #[serde(default)]
    pub name: Option<String>,
}

method!(
    /// Returns every output.
    Outputs, "outputs", NoParams => Vec<OutputState>
);
method!(
    /// Changes an output and returns its new state.
    SetOutput, "set_output", SetOutputParams => OutputState
);
method!(
    /// Adds an output and returns its state.
    AddOutput, "add_output", AddOutputParams => OutputState
);
method!(
    /// Removes an output and its `wl_output` global. Its windows move to the primary output.
    /// The session keeps its config for `replug_output`.
    RemoveOutput, "remove_output", RemoveOutputParams => ()
);
method!(
    /// Plugs an unplugged output in again with its config and returns its state.
    ReplugOutput, "replug_output", ReplugOutputParams => OutputState
);
method!(
    /// Returns the configs of the unplugged outputs, the latest last.
    UnpluggedOutputs, "unplugged_outputs", NoParams => Vec<OutputConfig>
);
