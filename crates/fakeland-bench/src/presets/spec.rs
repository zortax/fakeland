//! The typed side of a preset: its parameter type and its bench builder.

use schemars::JsonSchema;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use super::{Preset, PresetError, PresetParams};
use crate::runner::Bench;

/// A preset with typed parameters.
pub(crate) trait Spec {
    /// The name.
    const NAME: &'static str;
    /// The description.
    const DESCRIPTION: &'static str;
    /// The parameters; `Default` gives the defaults.
    type Params: Serialize + DeserializeOwned + JsonSchema + Default;
    /// Builds the bench.
    fn build(p: Self::Params) -> Result<Bench, PresetError>;
}

/// Returns the preset of a spec.
pub(crate) fn preset<S: Spec>() -> Preset {
    Preset {
        name: S::NAME,
        description: S::DESCRIPTION,
        schema: || schemars::schema_for!(S::Params),
        defaults: || serde_json::to_value(S::Params::default()).unwrap_or(Value::Null),
        build: build::<S>,
    }
}

fn build<S: Spec>(params: &PresetParams) -> Result<Bench, PresetError> {
    let schema = schemars::schema_for!(S::Params);
    let known = property_names(schema.as_value());
    if let Some(key) = params.0.keys().find(|k| !known.contains(k)) {
        return Err(PresetError::UnknownParam {
            preset: S::NAME.to_string(),
            key: key.clone(),
            known: known.join(", "),
        });
    }
    let typed: S::Params = serde_json::from_value(params.to_value())
        .map_err(|e| PresetError::params(S::NAME, e.to_string()))?;
    let effective = serde_json::to_value(&typed).unwrap_or(Value::Null);
    Ok(S::build(typed)?.preset(S::NAME, effective))
}

/// Returns the property names of an object schema, with those of `allOf` parts.
fn property_names(schema: &Value) -> Vec<String> {
    let mut out: Vec<String> = schema
        .get("properties")
        .and_then(Value::as_object)
        .map(|p| p.keys().cloned().collect())
        .unwrap_or_default();
    for key in ["allOf", "anyOf", "oneOf"] {
        if let Some(parts) = schema.get(key).and_then(Value::as_array) {
            for part in parts {
                out.extend(property_names(part));
            }
        }
    }
    out.sort();
    out.dedup();
    out
}
