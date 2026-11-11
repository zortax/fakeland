//! The bench harness writes reports, saves baselines and compares against them.

use fakeland_bench::compare::Outcome;
use fakeland_bench::report::{BenchConfig, Section};
use fakeland_bench::{
    BenchReport, Better, Harness, MetricSummary, REPORT_FORMAT_VERSION, Unit, Validity,
};
use fakeland_trace::Environment;
use fakeland_types::Timestamp;

fn report(p50: f64) -> BenchReport {
    BenchReport {
        format_version: REPORT_FORMAT_VERSION,
        name: "demo".into(),
        preset: Some("hover-latency".into()),
        params: None,
        environment: Environment::default(),
        config: BenchConfig::default(),
        started: Timestamp(0),
        finished: Timestamp(1_000_000_000),
        iterations: Vec::new(),
        validity: Validity::from_checks(Vec::new()),
        sections: vec![Section {
            name: "latency".into(),
            metrics: vec![
                MetricSummary::of(
                    "total_p50",
                    None,
                    Unit::Ms,
                    Better::Lower,
                    vec![p50, p50 + 0.1, p50 - 0.1],
                    1,
                )
                .unwrap(),
            ],
            samples: Some(3),
            pooled: Vec::new(),
            iterations: Vec::new(),
            error: None,
        }],
    }
}

#[test]
fn baseline_round_trip() {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("harness");
    let _ = std::fs::remove_dir_all(&dir);
    let mut save = Harness::from_env().out_dir(&dir).save_baseline(true);
    assert!(save.record("demo", &report(10.0)).is_none());
    assert!(dir.join("baseline/demo.json").exists());
    assert!(dir.join("demo.txt").exists());
    let mut check = Harness::from_env().out_dir(&dir).save_baseline(false);
    let same = check.record("demo", &report(10.0)).unwrap();
    assert_eq!(same.outcome, Outcome::Unchanged);
    let worse = check.record("demo", &report(14.0)).unwrap();
    assert_eq!(worse.outcome, Outcome::Regressed);
    assert_eq!(check.regressions(), ["demo"]);
    let text = std::fs::read_to_string(dir.join("demo.json")).unwrap();
    assert_eq!(BenchReport::from_json(&text).unwrap(), report(14.0));
}
