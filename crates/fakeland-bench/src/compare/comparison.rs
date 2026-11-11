//! The comparison of two reports.

use serde::{Deserialize, Serialize};

use super::Thresholds;
use crate::metrics::{Better, MetricSummary, Unit};
use crate::report::BenchReport;

/// The verdict of one metric.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The metric got better beyond its threshold.
    Improved,
    /// The metric got worse beyond its threshold.
    Regressed,
    /// The change stays within the threshold.
    Unchanged,
    /// The change passes the threshold, and the intervals overlap.
    Inconclusive,
    /// A metric without a better direction changed beyond its threshold.
    Changed,
    /// Only the current report has the metric.
    Added,
    /// Only the baseline has the metric.
    Removed,
    /// A threshold ignores the metric.
    Ignored,
}

/// One metric of both reports.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MetricComparison {
    /// The section.
    pub section: String,
    /// The metric.
    pub name: String,
    /// The variant.
    pub variant: Option<String>,
    /// The unit.
    pub unit: Unit,
    /// The better direction.
    pub better: Better,
    /// The baseline mean.
    pub baseline: Option<f64>,
    /// The current mean.
    pub current: Option<f64>,
    /// Current minus baseline.
    pub delta: Option<f64>,
    /// The delta over the baseline magnitude.
    pub relative: Option<f64>,
    /// The smallest delta that counts.
    pub limit: f64,
    /// The intervals of both sides overlap; `None` when one side has no interval.
    pub ci_overlap: Option<bool>,
    /// The verdict.
    pub verdict: Verdict,
}

/// The overall outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// At least one metric regressed.
    Regressed,
    /// No metric regressed and at least one improved.
    Improved,
    /// No metric passed its threshold.
    Unchanged,
}

/// A report against its baseline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Comparison {
    /// The baseline bench name.
    pub baseline: String,
    /// The current bench name.
    pub current: String,
    /// The overall outcome.
    pub outcome: Outcome,
    /// The count of regressed metrics.
    pub regressions: usize,
    /// The count of improved metrics.
    pub improvements: usize,
    /// The baseline passed its validity checks.
    pub baseline_valid: bool,
    /// The current report passed its validity checks.
    pub current_valid: bool,
    /// Differences that make the reports hard to compare, such as another GPU driver.
    pub warnings: Vec<String>,
    /// The thresholds.
    pub thresholds: Thresholds,
    /// Every metric of either report.
    pub metrics: Vec<MetricComparison>,
}

impl Comparison {
    /// Returns true when a metric regressed.
    pub fn is_regression(&self) -> bool {
        self.outcome == Outcome::Regressed
    }

    /// Returns the metrics with a verdict.
    pub fn with_verdict(&self, verdict: Verdict) -> impl Iterator<Item = &MetricComparison> + '_ {
        self.metrics.iter().filter(move |m| m.verdict == verdict)
    }
}

/// Compares a report against a baseline, metric by metric, matched by section, name and variant.
pub fn compare(
    baseline: &BenchReport,
    current: &BenchReport,
    thresholds: &Thresholds,
) -> Comparison {
    let mut metrics = Vec::new();
    for section in &current.sections {
        let base_section = baseline.section(&section.name);
        for m in &section.metrics {
            let base = base_section.and_then(|s| s.metric(&m.name, m.variant.as_deref()));
            metrics.push(one(&section.name, base, Some(m), thresholds));
        }
    }
    for section in &baseline.sections {
        let cur_section = current.section(&section.name);
        for m in &section.metrics {
            if cur_section
                .and_then(|s| s.metric(&m.name, m.variant.as_deref()))
                .is_none()
            {
                metrics.push(one(&section.name, Some(m), None, thresholds));
            }
        }
    }
    let regressions = metrics
        .iter()
        .filter(|m| m.verdict == Verdict::Regressed)
        .count();
    let improvements = metrics
        .iter()
        .filter(|m| m.verdict == Verdict::Improved)
        .count();
    Comparison {
        baseline: baseline.name.clone(),
        current: current.name.clone(),
        outcome: if regressions > 0 {
            Outcome::Regressed
        } else if improvements > 0 {
            Outcome::Improved
        } else {
            Outcome::Unchanged
        },
        regressions,
        improvements,
        baseline_valid: baseline.is_valid(),
        current_valid: current.is_valid(),
        warnings: warnings(baseline, current),
        thresholds: thresholds.clone(),
        metrics,
    }
}

fn one(
    section: &str,
    base: Option<&MetricSummary>,
    cur: Option<&MetricSummary>,
    t: &Thresholds,
) -> MetricComparison {
    let any = cur.or(base).expect("one side exists");
    let full = format!("{section}.{}", any.name);
    let (relative, absolute, ignore) = t.resolve(&full);
    let mut out = MetricComparison {
        section: section.to_string(),
        name: any.name.clone(),
        variant: any.variant.clone(),
        unit: any.unit,
        better: any.better,
        baseline: base.map(|b| b.mean),
        current: cur.map(|c| c.mean),
        delta: None,
        relative: None,
        limit: 0.0,
        ci_overlap: None,
        verdict: Verdict::Unchanged,
    };
    let (Some(b), Some(c)) = (base, cur) else {
        out.verdict = if cur.is_some() {
            Verdict::Added
        } else {
            Verdict::Removed
        };
        return out;
    };
    let delta = c.mean - b.mean;
    out.delta = Some(delta);
    out.relative = (b.mean.abs() > NEAR_ZERO).then(|| delta / b.mean.abs());
    out.limit = (relative * b.mean.abs()).max(absolute);
    out.ci_overlap = b.ci.zip(c.ci).map(|(x, y)| x.overlaps(&y));
    out.verdict = if ignore {
        Verdict::Ignored
    } else if delta.abs() <= out.limit {
        Verdict::Unchanged
    } else if t.require_ci_separation && out.ci_overlap == Some(true) {
        Verdict::Inconclusive
    } else {
        match (c.better, delta < 0.0) {
            (Better::Lower, true) | (Better::Higher, false) => Verdict::Improved,
            (Better::Lower, false) | (Better::Higher, true) => Verdict::Regressed,
            (Better::Neither, _) => Verdict::Changed,
        }
    };
    out
}

/// Baseline means at or below this size count as zero and get no relative change. Metric means
/// of zero come out of float sums with a remainder of about 1e-16.
const NEAR_ZERO: f64 = 1e-9;

fn warnings(a: &BenchReport, b: &BenchReport) -> Vec<String> {
    let mut out = Vec::new();
    let (ea, eb) = (&a.environment, &b.environment);
    for (what, x, y) in [
        ("gpu", &ea.gpu, &eb.gpu),
        ("driver", &ea.driver, &eb.driver),
        ("kernel", &ea.kernel, &eb.kernel),
        ("cpu", &ea.cpu, &eb.cpu),
        ("governor", &ea.governor, &eb.governor),
        ("renderer", &ea.renderer, &eb.renderer),
    ] {
        if x != y {
            out.push(format!("{what} differs: {x:?} vs {y:?}"));
        }
    }
    if a.preset != b.preset {
        out.push(format!("preset differs: {:?} vs {:?}", a.preset, b.preset));
    }
    if a.params != b.params {
        out.push("preset parameters differ".into());
    }
    let modes = |r: &BenchReport| -> Vec<String> {
        r.config
            .outputs
            .iter()
            .map(|o| {
                format!(
                    "{} {}x{}@{} x{}",
                    o.name, o.mode.width, o.mode.height, o.mode.refresh.0, o.scale
                )
            })
            .collect()
    };
    if modes(a) != modes(b) {
        out.push(format!("outputs differ: {:?} vs {:?}", modes(a), modes(b)));
    }
    if !a.is_valid() {
        out.push("the baseline failed its validity checks".into());
    }
    if !b.is_valid() {
        out.push("the current report failed its validity checks".into());
    }
    out
}
