//! The scenario document and its JSON form.

use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{ScenarioBuilder, ScenarioError, Step};
use crate::Trigger;

/// A named scenario: steps in order, triggers that stay armed while the scenario runs, and an
/// optional overall deadline.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Scenario {
    /// The name.
    pub name: String,
    /// The steps in order.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// The triggers.
    #[serde(default)]
    pub triggers: Vec<Trigger>,
    /// The deadline of the whole scenario.
    #[serde(default)]
    pub timeout: Option<Dur>,
}

impl Scenario {
    /// Starts a builder.
    pub fn builder(name: impl Into<String>) -> ScenarioBuilder {
        ScenarioBuilder::new(name)
    }

    /// Parses a scenario from JSON and validates it.
    pub fn from_json(json: &str) -> Result<Self, ScenarioError> {
        let s: Self = serde_json::from_str(json).map_err(|e| ScenarioError::Json(e.to_string()))?;
        s.validate()?;
        Ok(s)
    }

    /// Returns the scenario as indented JSON.
    pub fn to_json_pretty(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    /// Returns the JSON schema of scenarios.
    pub fn json_schema() -> schemars::Schema {
        schemars::schema_for!(Scenario)
    }
}
