//! Preset parameters as a JSON object.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// The parameters of a preset: a JSON object whose missing fields take the preset defaults.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct PresetParams(pub Map<String, Value>);

impl PresetParams {
    /// Builds empty parameters, which select every default.
    pub fn new() -> Self {
        Self::default()
    }

    /// Parses parameters from a JSON object.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Builds parameters from a JSON value. A value other than an object or null gives `None`.
    pub fn from_value(value: Value) -> Option<Self> {
        match value {
            Value::Object(m) => Some(Self(m)),
            Value::Null => Some(Self::default()),
            _ => None,
        }
    }

    /// Sets one parameter.
    pub fn set(mut self, key: impl Into<String>, value: impl Serialize) -> Self {
        self.0.insert(
            key.into(),
            serde_json::to_value(value).unwrap_or(Value::Null),
        );
        self
    }

    /// Parses `key=value` pairs. A value that parses as JSON is JSON, else a string, so
    /// `iterations=5`, `rates=[60,144]` and `text=hello` all work.
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = &'a str>) -> Result<Self, String> {
        let mut out = Self::default();
        for pair in pairs {
            let (k, v) = pair
                .split_once('=')
                .ok_or_else(|| format!("{pair:?} is not key=value"))?;
            let value = serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.to_string()));
            out.0.insert(k.trim().to_string(), value);
        }
        Ok(out)
    }

    /// Returns the parameters as a JSON value.
    pub fn to_value(&self) -> Value {
        Value::Object(self.0.clone())
    }
}
