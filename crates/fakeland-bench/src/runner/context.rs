//! What a driver knows about the iteration it runs.

use fakeland::{OutputChange, WindowInfo};
use fakeland_scenario::WindowRef;
use fakeland_types::{Nanos, OutputId};
use serde::de::DeserializeOwned;

/// A variant of a bench: output changes that apply before its iterations and free values that
/// drivers read. Every variant runs the warmup and measured iterations.
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {
    /// The label in reports, such as `144Hz`.
    pub label: String,
    /// Output changes; `None` selects the output of the target, else the primary output.
    pub outputs: Vec<(Option<OutputId>, OutputChange)>,
    /// Values for drivers, such as a scroll velocity.
    pub values: serde_json::Value,
}

impl Variant {
    /// Builds a variant without changes.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            outputs: Vec::new(),
            values: serde_json::Value::Null,
        }
    }

    /// Adds a change of the target output.
    pub fn output(mut self, change: OutputChange) -> Self {
        self.outputs.push((None, change));
        self
    }

    /// Adds a change of one output.
    pub fn output_on(mut self, output: OutputId, change: OutputChange) -> Self {
        self.outputs.push((Some(output), change));
        self
    }

    /// Sets the driver values.
    pub fn values(mut self, values: serde_json::Value) -> Self {
        self.values = values;
        self
    }
}

/// The iteration that a driver runs.
#[derive(Clone, Debug)]
pub struct IterationContext {
    /// The index over every iteration of the bench, warmups included, from 0.
    pub index: u32,
    /// The index over measured iterations of the variant; `None` for a warmup.
    pub measured: Option<u32>,
    /// The variant.
    pub variant: Option<Variant>,
    /// The target window as it is when the iteration starts.
    pub target: Option<WindowInfo>,
    /// The planned vblank phase of the first input.
    pub phase: Option<Nanos>,
    /// The refresh period of the target output.
    pub period: Option<Nanos>,
    /// The seed of the iteration.
    pub seed: u64,
}

impl IterationContext {
    /// Returns true for a warmup.
    pub fn is_warmup(&self) -> bool {
        self.measured.is_none()
    }

    /// Returns a matcher of the target window, or the focused window without a target.
    pub fn window(&self) -> WindowRef {
        self.target
            .as_ref()
            .map_or(WindowRef::Focused, |w| WindowRef::id(w.id))
    }

    /// Returns the variant label.
    pub fn variant_label(&self) -> Option<&str> {
        self.variant.as_ref().map(|v| v.label.as_str())
    }

    /// Reads the variant values as a type.
    pub fn variant_values<T: DeserializeOwned>(&self) -> Option<T> {
        let v = self.variant.as_ref()?;
        serde_json::from_value(v.values.clone()).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variant_values_and_window() {
        #[derive(serde::Deserialize, PartialEq, Debug)]
        struct V {
            velocity: f64,
        }
        let cx = IterationContext {
            index: 3,
            measured: None,
            variant: Some(Variant::new("fast").values(serde_json::json!({"velocity": 2400.0}))),
            target: None,
            phase: None,
            period: None,
            seed: 0,
        };
        assert!(cx.is_warmup());
        assert_eq!(cx.window(), WindowRef::Focused);
        assert_eq!(cx.variant_label(), Some("fast"));
        assert_eq!(cx.variant_values::<V>(), Some(V { velocity: 2400.0 }));
    }
}
