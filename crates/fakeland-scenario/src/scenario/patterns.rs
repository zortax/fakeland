//! Validation of title patterns. Every `title_regex` window reference in a scenario compiles with
//! the regex syntax of the compositor. The walk runs over the JSON form of the scenario, so it
//! reaches window references in positions, regions, waits, triggers and window actions alike.

use serde_json::Value;

use super::{Scenario, ScenarioError};

/// The JSON tag of a title pattern window reference.
const TITLE_REGEX: &str = "title_regex";

impl Scenario {
    /// Checks that every title pattern compiles.
    pub(super) fn validate_patterns(&self) -> Result<(), ScenarioError> {
        let value = serde_json::to_value(self).map_err(|e| ScenarioError::Json(e.to_string()))?;
        walk("", &value)
    }
}

fn walk(location: &str, value: &Value) -> Result<(), ScenarioError> {
    match value {
        Value::Object(map) => {
            if map.get("type").and_then(Value::as_str) == Some(TITLE_REGEX)
                && let Some(pattern) = map.get("pattern").and_then(Value::as_str)
                && let Err(e) = regex::Regex::new(pattern)
            {
                return Err(ScenarioError::Invalid {
                    location: location.to_owned(),
                    message: format!("the title pattern does not compile: {e}"),
                });
            }
            for (key, child) in map {
                let here = if location.is_empty() {
                    key.clone()
                } else {
                    format!("{location}.{key}")
                };
                walk(&here, child)?;
            }
            Ok(())
        }
        Value::Array(items) => {
            for (i, child) in items.iter().enumerate() {
                walk(&format!("{location}[{i}]"), child)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use crate::prelude::*;

    #[test]
    fn bad_title_patterns_fail_with_their_location() {
        let ok = Scenario::builder("ok")
            .wait_for_window(WindowRef::title_regex("^Pick .*$"))
            .build();
        assert!(ok.is_ok(), "{ok:?}");
        let bad = Scenario::builder("bad")
            .mark("first")
            .click(Position::center(WindowRef::title_regex("(unclosed")))
            .build_unchecked();
        match bad.validate() {
            Err(ScenarioError::Invalid { location, message }) => {
                assert!(location.starts_with("steps[1]"), "{location}");
                assert!(message.contains("title pattern"), "{message}");
            }
            other => panic!("expected a pattern error, got {other:?}"),
        }
    }
}
