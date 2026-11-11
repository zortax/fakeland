//! Aggregation of metrics over iterations: summary values and a bootstrap confidence interval of
//! the mean.

use fakeland_trace::stats::{ConfidenceInterval, median};
use serde::{Deserialize, Serialize};

use super::{Better, Metric, Unit};

/// The resamples of a bootstrap interval.
const RESAMPLES: usize = 2000;

/// The confidence level of the intervals.
const CONFIDENCE: f64 = 0.95;

/// The smallest sample count with an interval.
const MIN_FOR_CI: usize = 3;

/// A confidence interval.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    /// The lower bound.
    pub lo: f64,
    /// The upper bound.
    pub hi: f64,
    /// The confidence level in `0..1`.
    pub confidence: f64,
}

impl Interval {
    /// Returns true when two intervals share a value.
    pub fn overlaps(&self, other: &Self) -> bool {
        self.lo <= other.hi && other.lo <= self.hi
    }
}

/// One metric over every measured iteration of one variant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricSummary {
    /// The metric name.
    pub name: String,
    /// The variant label, when the bench has variants.
    #[serde(default)]
    pub variant: Option<String>,
    /// The unit.
    pub unit: Unit,
    /// The better direction.
    pub better: Better,
    /// The count of iterations that reported the metric.
    pub n: usize,
    /// The mean over iterations. Comparisons use this value.
    pub mean: f64,
    /// The median over iterations.
    pub median: f64,
    /// The sample standard deviation over iterations; 0 for one iteration.
    pub stddev: f64,
    /// The smallest value.
    pub min: f64,
    /// The largest value.
    pub max: f64,
    /// The 95% bootstrap interval of the mean, from three iterations on.
    #[serde(default)]
    pub ci: Option<Interval>,
    /// The value of each iteration in order.
    pub values: Vec<f64>,
}

impl MetricSummary {
    /// Summarizes the values of one metric. Returns `None` without a finite value.
    pub fn of(
        name: &str,
        variant: Option<String>,
        unit: Unit,
        better: Better,
        values: Vec<f64>,
        seed: u64,
    ) -> Option<Self> {
        let finite: Vec<f64> = values.into_iter().filter(|v| v.is_finite()).collect();
        if finite.is_empty() {
            return None;
        }
        let n = finite.len();
        let mean = mean(&finite);
        let stddev = if n > 1 {
            (finite.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / (n - 1) as f64).sqrt()
        } else {
            0.0
        };
        let ci = (n >= MIN_FOR_CI)
            .then(|| ConfidenceInterval::bootstrap(&finite, mean_of, RESAMPLES, CONFIDENCE, seed))
            .flatten()
            .map(|c| Interval {
                lo: c.lo,
                hi: c.hi,
                confidence: c.confidence,
            });
        Some(Self {
            name: name.to_string(),
            variant,
            unit,
            better,
            n,
            mean,
            median: median(&finite).unwrap_or(mean),
            stddev,
            min: finite.iter().copied().fold(f64::INFINITY, f64::min),
            max: finite.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            ci,
            values: finite,
        })
    }
}

fn mean(values: &[f64]) -> f64 {
    values.iter().sum::<f64>() / values.len() as f64
}

fn mean_of(values: &[f64]) -> f64 {
    mean(values)
}

/// Aggregates the metrics of several iterations. Each item is the variant of an iteration and its
/// metrics. The result keeps the order in which names first appear, grouped by variant in the
/// order variants first appear.
pub fn aggregate(iterations: &[(Option<String>, Vec<Metric>)], seed: u64) -> Vec<MetricSummary> {
    let mut keys: Vec<(Option<String>, String, Unit, Better)> = Vec::new();
    for (variant, metrics) in iterations {
        for m in metrics {
            if !keys.iter().any(|(v, n, _, _)| v == variant && *n == m.name) {
                keys.push((variant.clone(), m.name.clone(), m.unit, m.better));
            }
        }
    }
    let mut variants: Vec<Option<String>> = Vec::new();
    for (v, ..) in &keys {
        if !variants.contains(v) {
            variants.push(v.clone());
        }
    }
    let mut out = Vec::new();
    for variant in &variants {
        for (i, (v, name, unit, better)) in keys.iter().enumerate() {
            if v != variant {
                continue;
            }
            let values: Vec<f64> = iterations
                .iter()
                .filter(|(iv, _)| iv == variant)
                .filter_map(|(_, ms)| ms.iter().find(|m| m.name == *name).map(|m| m.value))
                .collect();
            let salt = seed.wrapping_add(i as u64);
            if let Some(s) = MetricSummary::of(name, v.clone(), *unit, *better, values, salt) {
                out.push(s);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(name: &str, v: f64) -> Metric {
        Metric::new(name, v, Unit::Ms, Better::Lower)
    }

    #[test]
    fn summary_values() {
        let s = MetricSummary::of(
            "x",
            None,
            Unit::Ms,
            Better::Lower,
            vec![1.0, 2.0, 3.0, 4.0, f64::NAN],
            7,
        )
        .unwrap();
        assert_eq!(s.n, 4);
        assert_eq!(s.mean, 2.5);
        assert_eq!(s.median, 2.5);
        assert!((s.stddev - 1.2909944).abs() < 1e-6);
        assert_eq!((s.min, s.max), (1.0, 4.0));
        let ci = s.ci.unwrap();
        assert!(ci.lo >= 1.0 && ci.hi <= 4.0 && ci.lo < 2.5 && ci.hi > 2.5);
        let again = MetricSummary::of(
            "x",
            None,
            Unit::Ms,
            Better::Lower,
            vec![1.0, 2.0, 3.0, 4.0],
            7,
        );
        assert_eq!(
            again.unwrap().ci,
            s.ci,
            "the same seed gives the same interval"
        );
        let one = MetricSummary::of("x", None, Unit::Ms, Better::Lower, vec![5.0], 0).unwrap();
        assert_eq!((one.stddev, one.ci), (0.0, None));
        assert!(MetricSummary::of("x", None, Unit::Ms, Better::Lower, vec![], 0).is_none());
    }

    #[test]
    fn aggregate_groups_by_variant_and_name() {
        let a = Some("60Hz".to_string());
        let b = Some("120Hz".to_string());
        let its = vec![
            (a.clone(), vec![m("p50", 10.0), m("p99", 20.0)]),
            (a.clone(), vec![m("p50", 12.0)]),
            (b.clone(), vec![m("p50", 5.0), m("p99", 9.0)]),
        ];
        let s = aggregate(&its, 1);
        let keys: Vec<(Option<&str>, &str, usize)> = s
            .iter()
            .map(|s| (s.variant.as_deref(), s.name.as_str(), s.n))
            .collect();
        assert_eq!(
            keys,
            [
                (Some("60Hz"), "p50", 2),
                (Some("60Hz"), "p99", 1),
                (Some("120Hz"), "p50", 1),
                (Some("120Hz"), "p99", 1)
            ]
        );
        assert_eq!(s[0].mean, 11.0);
        assert!(
            Interval {
                lo: 0.0,
                hi: 1.0,
                confidence: 0.95
            }
            .overlaps(&Interval {
                lo: 1.0,
                hi: 2.0,
                confidence: 0.95
            })
        );
    }
}
