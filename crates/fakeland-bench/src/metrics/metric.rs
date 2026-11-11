//! One scalar metric with its unit and the direction that counts as better.

use fakeland_types::Nanos;
use serde::{Deserialize, Serialize};

/// The unit of a metric value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// Milliseconds.
    Ms,
    /// Logical pixels.
    Px,
    /// Logical pixels per second.
    PxPerS,
    /// Frames per second.
    Fps,
    /// A count of events.
    Count,
    /// A plain ratio, 1 means equal.
    Ratio,
    /// A fraction in `0..=1`.
    Fraction,
    /// Percent of one CPU or of a device.
    Percent,
    /// Bytes.
    Bytes,
    /// A score in `0..=100`.
    Score,
}

impl Unit {
    /// Returns the short label for tables.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ms => "ms",
            Self::Px => "px",
            Self::PxPerS => "px/s",
            Self::Fps => "fps",
            Self::Count => "",
            Self::Ratio => "x",
            Self::Fraction => "frac",
            Self::Percent => "%",
            Self::Bytes => "B",
            Self::Score => "pts",
        }
    }
}

/// Which change of a metric is an improvement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Better {
    /// A lower value is better, for example a latency.
    Lower,
    /// A higher value is better, for example a frame rate.
    Higher,
    /// The metric describes the run and has no better direction.
    Neither,
}

/// One named value of a report.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Metric {
    /// The name, unique within its report, such as `total_p50`.
    pub name: String,
    /// The value.
    pub value: f64,
    /// The unit.
    pub unit: Unit,
    /// The better direction.
    pub better: Better,
}

impl Metric {
    /// Builds a metric.
    pub fn new(name: impl Into<String>, value: f64, unit: Unit, better: Better) -> Self {
        Self {
            name: name.into(),
            value,
            unit,
            better,
        }
    }
}

/// A list of metrics with helpers that skip absent and non-finite values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MetricList(pub Vec<Metric>);

impl MetricList {
    /// Builds an empty list.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a metric when the value is present and finite.
    pub fn push(&mut self, name: &str, value: Option<f64>, unit: Unit, better: Better) {
        if let Some(v) = value.filter(|v| v.is_finite()) {
            self.0.push(Metric::new(name, v, unit, better));
        }
    }

    /// Adds a lower-is-better metric.
    pub fn lower(&mut self, name: &str, value: Option<f64>, unit: Unit) {
        self.push(name, value, unit, Better::Lower);
    }

    /// Adds a higher-is-better metric.
    pub fn higher(&mut self, name: &str, value: Option<f64>, unit: Unit) {
        self.push(name, value, unit, Better::Higher);
    }

    /// Adds a metric without a better direction.
    pub fn neutral(&mut self, name: &str, value: Option<f64>, unit: Unit) {
        self.push(name, value, unit, Better::Neither);
    }

    /// Adds a lower-is-better count.
    pub fn count(&mut self, name: &str, value: usize) {
        self.lower(name, Some(value as f64), Unit::Count);
    }

    /// Adds a lower-is-better span in milliseconds.
    pub fn ms(&mut self, name: &str, value: Option<Nanos>) {
        self.lower(name, value.map(Nanos::as_millis_f64), Unit::Ms);
    }

    /// Returns the metrics.
    pub fn into_vec(self) -> Vec<Metric> {
        self.0
    }
}

/// The metrics of a report. Every analyzer report implements it.
pub trait Metrics {
    /// Returns the scalar metrics of the report, in display order.
    fn metrics(&self) -> Vec<Metric> {
        Vec::new()
    }

    /// Returns the count of samples behind the report, when the report has samples.
    fn samples(&self) -> Option<usize> {
        None
    }
}

impl Metrics for serde_json::Value {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_skips_missing_and_non_finite_values() {
        let mut m = MetricList::new();
        m.lower("a", Some(1.0), Unit::Ms);
        m.lower("b", None, Unit::Ms);
        m.higher("c", Some(f64::NAN), Unit::Fps);
        m.ms("d", Some(Nanos::from_micros(1500)));
        m.count("e", 3);
        let v = m.into_vec();
        let names: Vec<&str> = v.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["a", "d", "e"]);
        assert_eq!(v[1].value, 1.5);
        assert_eq!(v[2].better, Better::Lower);
    }

    #[test]
    fn units_serialize_in_snake_case() {
        assert_eq!(
            serde_json::to_string(&Unit::PxPerS).unwrap(),
            "\"px_per_s\""
        );
        assert_eq!(Unit::Ms.label(), "ms");
    }
}
