//! Comparisons of synthetic reports.

use fakeland_trace::Environment;
use fakeland_types::Timestamp;

use super::*;
use crate::metrics::{Better, MetricSummary, Unit};
use crate::report::{BenchConfig, BenchReport, REPORT_FORMAT_VERSION, Section};
use crate::validity::Validity;

fn summary(name: &str, better: Better, values: Vec<f64>) -> MetricSummary {
    MetricSummary::of(name, None, Unit::Ms, better, values, 1).unwrap()
}

fn report(metrics: Vec<MetricSummary>) -> BenchReport {
    BenchReport {
        format_version: REPORT_FORMAT_VERSION,
        name: "b".into(),
        preset: None,
        params: None,
        environment: Environment::default(),
        config: BenchConfig::default(),
        started: Timestamp(0),
        finished: Timestamp(1),
        iterations: Vec::new(),
        validity: Validity::from_checks(Vec::new()),
        sections: vec![Section {
            name: "latency".into(),
            metrics,
            samples: None,
            pooled: Vec::new(),
            iterations: Vec::new(),
            error: None,
        }],
    }
}

#[test]
fn verdicts_follow_direction_threshold_and_intervals() {
    let base = report(vec![
        summary("total_p50", Better::Lower, vec![10.0, 10.2, 9.8, 10.1, 9.9]),
        summary("fps", Better::Higher, vec![60.0, 60.0, 60.0]),
        summary("noisy", Better::Lower, vec![5.0, 15.0, 10.0]),
        summary("gone", Better::Lower, vec![1.0]),
        summary("count", Better::Neither, vec![3.0]),
    ]);
    let cur = report(vec![
        summary(
            "total_p50",
            Better::Lower,
            vec![12.0, 12.2, 11.8, 12.1, 11.9],
        ),
        summary("fps", Better::Higher, vec![60.5, 60.4, 60.6]),
        summary("noisy", Better::Lower, vec![14.0, 6.0, 12.0]),
        summary("count", Better::Neither, vec![9.0]),
        summary("new", Better::Lower, vec![1.0]),
    ]);
    let c = compare(&base, &cur, &Thresholds::default());
    let v = |n: &str| c.metrics.iter().find(|m| m.name == n).unwrap().verdict;
    assert_eq!(v("total_p50"), Verdict::Regressed);
    assert_eq!(v("fps"), Verdict::Unchanged, "under 5%");
    assert_eq!(v("noisy"), Verdict::Inconclusive);
    assert_eq!(v("count"), Verdict::Changed);
    assert_eq!(v("new"), Verdict::Added);
    assert_eq!(v("gone"), Verdict::Removed);
    assert_eq!(c.outcome, Outcome::Regressed);
    assert!(c.is_regression());
    let total = c.metrics.iter().find(|m| m.name == "total_p50").unwrap();
    assert!((total.delta.unwrap() - 2.0).abs() < 1e-9);
    assert!((total.relative.unwrap() - 0.2).abs() < 1e-9);
    let text = c.to_text();
    assert!(text.contains("REGRESSED") && text.contains("total_p50"));

    let back = compare(&cur, &base, &Thresholds::default());
    assert_eq!(back.outcome, Outcome::Improved);

    let tolerant = Thresholds::default().with(MetricThreshold {
        pattern: "latency.total_*".into(),
        relative: Some(0.5),
        ..MetricThreshold::default()
    });
    let c = compare(&base, &cur, &tolerant);
    assert_eq!(c.outcome, Outcome::Unchanged);
    let ignoring = Thresholds::default().with(MetricThreshold {
        pattern: "*".into(),
        ignore: true,
        ..MetricThreshold::default()
    });
    assert!(
        compare(&base, &cur, &ignoring)
            .metrics
            .iter()
            .filter(|m| m.baseline.is_some() && m.current.is_some())
            .all(|m| m.verdict == Verdict::Ignored)
    );
}

#[test]
fn environment_differences_warn() {
    let base = report(vec![]);
    let mut cur = report(vec![]);
    cur.environment.driver = "610.43".into();
    let c = compare(&base, &cur, &Thresholds::default());
    assert_eq!(c.warnings.len(), 1);
    assert!(c.warnings[0].contains("driver"));
}

#[test]
fn a_zero_baseline_has_no_relative_change() {
    let base = report(vec![summary("jitter", Better::Lower, vec![3e-16])]);
    let cur = report(vec![summary("jitter", Better::Lower, vec![0.5])]);
    let c = compare(&base, &cur, &Thresholds::default());
    let jitter = &c.metrics[0];
    assert_eq!(jitter.relative, None);
    assert_eq!(jitter.verdict, Verdict::Regressed);
}
