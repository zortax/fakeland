//! The event log: one JSON object per line on stdout, flushed at once.

use std::io::Write;

use serde_json::{Value, json};

/// Writes one event with its fields.
pub fn event(name: &str, fields: Value) {
    let mut line = json!({ "event": name });
    if let (Some(obj), Value::Object(extra)) = (line.as_object_mut(), fields) {
        obj.extend(extra);
    }
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}
