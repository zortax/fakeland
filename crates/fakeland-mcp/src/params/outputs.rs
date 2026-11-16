//! Parameters of the output tools.

use schemars::JsonSchema;
use serde::Deserialize;

/// Output changes. Give at least one.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetOutputParams {
    /// The output id. The default is the primary output.
    #[serde(default)]
    pub output: Option<u32>,
    /// A mode as `WIDTHxHEIGHT[@HZ]`, such as `2560x1440@144`.
    #[serde(default)]
    pub mode: Option<String>,
    /// A scale, such as 1.5.
    #[serde(default)]
    pub scale: Option<f64>,
    /// A timing model: `fixed`, `vrr:MIN-MAX`, `unthrottled`, `manual` or `manual:HZ`.
    #[serde(default)]
    pub timing: Option<String>,
    /// Power the output on or off.
    #[serde(default)]
    pub enabled: Option<bool>,
}

/// A new output, or an unplugged output to plug in again.
#[derive(Debug, Default, Deserialize, JsonSchema)]
pub struct AddOutputParams {
    /// A new output as `[NAME:]WIDTHxHEIGHT[@HZ]`, such as `HDMI-A-1:2560x1440@144`. The
    /// default name is the first free `DP-N`. Leave it out to plug in an unplugged output again.
    #[serde(default)]
    pub spec: Option<String>,
    /// The scale of a new output, such as 1.5.
    #[serde(default)]
    pub scale: Option<f64>,
    /// The position `[x, y]` of a new output in the global space. The default is right of the
    /// rightmost output.
    #[serde(default)]
    pub position: Option<[i32; 2]>,
    /// The connector name of the unplugged output to plug in again when `spec` is left out. The
    /// default is the latest unplugged output.
    #[serde(default)]
    pub name: Option<String>,
}

/// An output to unplug.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct RemoveOutputParams {
    /// The output id.
    pub output: u32,
}
