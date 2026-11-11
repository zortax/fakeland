//! Node ids: an AT-SPI object is a bus name plus an object path. The compact text form drops the
//! leading colon of a unique name and the common path prefix, so `:1.23` with
//! `/org/a11y/atspi/accessible/42` reads `1.23:42`.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::A11yError;

/// The path prefix of AT-SPI objects.
pub const PATH_PREFIX: &str = "/org/a11y/atspi/accessible/";

/// The path of the root object of every app and of the registry.
pub const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";

/// The bus name of the registry.
pub const REGISTRY: &str = "org.a11y.atspi.Registry";

/// One accessible object.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeRef {
    /// The bus name of the app, for example `:1.23`.
    pub bus: String,
    /// The object path.
    pub path: String,
}

impl NodeRef {
    /// Builds a reference.
    pub fn new(bus: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            bus: bus.into(),
            path: path.into(),
        }
    }

    /// Returns the root object of the registry.
    pub fn registry_root() -> Self {
        Self::new(REGISTRY, ROOT_PATH)
    }

    /// Returns true for the null reference that some apps send for a missing object.
    pub fn is_null(&self) -> bool {
        self.bus.is_empty() || self.path == "/org/a11y/atspi/null"
    }
}

impl fmt::Display for NodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bus = self.bus.strip_prefix(':').unwrap_or(&self.bus);
        let path = self.path.strip_prefix(PATH_PREFIX).unwrap_or(&self.path);
        write!(f, "{bus}:{path}")
    }
}

impl FromStr for NodeRef {
    type Err = A11yError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || A11yError::InvalidId(s.to_string());
        let s = s.trim();
        let s = s.strip_prefix(':').unwrap_or(s);
        let (bus, path) = s.split_once(':').ok_or_else(invalid)?;
        if bus.is_empty() || path.is_empty() {
            return Err(invalid());
        }
        let bus = if bus.starts_with(|c: char| c.is_ascii_digit()) {
            format!(":{bus}")
        } else {
            bus.to_string()
        };
        let path = if path.starts_with('/') {
            path.to_string()
        } else {
            format!("{PATH_PREFIX}{path}")
        };
        Ok(Self { bus, path })
    }
}

impl Serialize for NodeRef {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for NodeRef {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl JsonSchema for NodeRef {
    fn schema_name() -> Cow<'static, str> {
        "NodeRef".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "description": "An accessible node id as the tree prints it, for example \"1.23:42\".",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compact_form_round_trips() {
        let n = NodeRef::new(":1.23", "/org/a11y/atspi/accessible/42");
        assert_eq!(n.to_string(), "1.23:42");
        assert_eq!("1.23:42".parse::<NodeRef>().unwrap(), n);
        assert_eq!(":1.23:42".parse::<NodeRef>().unwrap(), n);
    }

    #[test]
    fn foreign_paths_and_well_known_names_keep_their_form() {
        let n = NodeRef::new(":1.7", "/org/gtk/Application/a11y/12");
        assert_eq!(n.to_string(), "1.7:/org/gtk/Application/a11y/12");
        assert_eq!(n.to_string().parse::<NodeRef>().unwrap(), n);
        let r = NodeRef::registry_root();
        assert_eq!(r.to_string(), "org.a11y.atspi.Registry:root");
        assert_eq!(r.to_string().parse::<NodeRef>().unwrap(), r);
    }

    #[test]
    fn rejects_malformed_ids() {
        for s in ["", "1.23", ":", "1.2:", ":x"] {
            assert!(s.parse::<NodeRef>().is_err(), "{s}");
        }
    }

    #[test]
    fn serde_uses_the_compact_form() {
        let n = NodeRef::new(":1.3", "/org/a11y/atspi/accessible/root");
        let json = serde_json::to_string(&n).unwrap();
        assert_eq!(json, "\"1.3:root\"");
        assert_eq!(serde_json::from_str::<NodeRef>(&json).unwrap(), n);
    }

    #[test]
    fn null_reference() {
        assert!(NodeRef::new(":1.3", "/org/a11y/atspi/null").is_null());
        assert!(NodeRef::new("", "/x").is_null());
        assert!(!NodeRef::new(":1.3", ROOT_PATH).is_null());
    }
}
