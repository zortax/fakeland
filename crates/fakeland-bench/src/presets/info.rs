//! A preset and its description.

use schemars::Schema;
use serde::Serialize;
use serde_json::Value;

use super::{PresetError, PresetParams};
use crate::runner::Bench;

/// The description of a preset for listings.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PresetInfo {
    /// The name, such as `hover-latency`.
    pub name: String,
    /// What the preset measures and what it needs from the app.
    pub description: String,
    /// The JSON schema of the parameters.
    pub params: Schema,
    /// The default parameters.
    pub defaults: Value,
}

/// A ready bench that parameters turn into a [`Bench`].
#[derive(Clone, Copy)]
pub struct Preset {
    pub(crate) name: &'static str,
    pub(crate) description: &'static str,
    pub(crate) schema: fn() -> Schema,
    pub(crate) defaults: fn() -> Value,
    pub(crate) build: fn(&PresetParams) -> Result<Bench, PresetError>,
}

impl std::fmt::Debug for Preset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Preset").field("name", &self.name).finish()
    }
}

impl Preset {
    /// Returns the name.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Returns the description.
    pub fn description(&self) -> &'static str {
        self.description
    }

    /// Returns the JSON schema of the parameters.
    pub fn params_schema(&self) -> Schema {
        (self.schema)()
    }

    /// Returns the default parameters.
    pub fn default_params(&self) -> Value {
        (self.defaults)()
    }

    /// Returns the listing entry.
    pub fn info(&self) -> PresetInfo {
        PresetInfo {
            name: self.name.to_string(),
            description: self.description.to_string(),
            params: self.params_schema(),
            defaults: self.default_params(),
        }
    }

    /// Builds the bench. Unknown parameter names and invalid values are errors.
    pub fn bench(&self, params: &PresetParams) -> Result<Bench, PresetError> {
        (self.build)(params)
    }
}
