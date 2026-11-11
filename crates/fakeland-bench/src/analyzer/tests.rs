//! Sections, pooling, variants and combinators on synthetic iterations.

use fakeland_types::Nanos;

use super::*;
use crate::analyzers::{Buffers, FramePacing};
use crate::input::{AnalysisInput, IterationMeta};
use crate::testing::Synth;

/// An iteration with `n` frames on every refresh, in a variant.
fn iteration(index: u32, n: u64, variant: Option<&str>) -> AnalysisInput {
    let mut s = Synth::new();
    for seq in 1..=n {
        s.frame(s.vblank(seq - 1) + Nanos::from_millis(4), seq);
    }
    let mut input = s.analysis();
    input.iteration = IterationMeta {
        index,
        measured: Some(index),
        variant: variant.map(String::from),
        ..input.iteration
    };
    input
}

#[test]
fn sections_aggregate_per_variant_and_pool() {
    let its = [
        iteration(0, 31, Some("a")),
        iteration(1, 61, Some("a")),
        iteration(2, 31, Some("b")),
    ];
    let refs: Vec<&AnalysisInput> = its.iter().collect();
    let set = AnalyzerSet::new()
        .with(FramePacing::default())
        .with(Buffers);
    assert_eq!(set.names(), ["frame_pacing", "buffers"]);
    let sections = set.sections(&refs, 1);
    let pacing = &sections[0];
    assert_eq!(pacing.iterations.len(), 3);
    assert_eq!(pacing.samples, Some(30 + 60 + 30));
    let fps_a = pacing.metric("fps", Some("a")).unwrap();
    assert_eq!(fps_a.n, 2);
    assert!((fps_a.mean - 60.0).abs() < 0.01);
    assert_eq!(pacing.metric("fps", Some("b")).unwrap().n, 1);
    assert!(pacing.metric("fps", None).is_none());
    assert_eq!(
        pacing.pooled.len(),
        1,
        "variant b has one iteration and does not pool"
    );
    assert_eq!(pacing.pooled[0].variant.as_deref(), Some("a"));
    assert_eq!(pacing.pooled[0].samples, Some(90));
    assert!(sections[1].pooled.is_empty());
    assert_eq!(sections[1].iterations[0].report["buffer_commits"], 31);
}

#[test]
fn combinators_rename_and_join() {
    let it = iteration(0, 11, None);
    let named = FramePacing::default().named("pacing_strict");
    assert_eq!(named.name(), "pacing_strict");
    let both = FramePacing::default().and(Buffers);
    assert_eq!(both.name(), "frame_pacing+buffers");
    let r = both.analyze(&it);
    let json = serde_json::to_value(&r).unwrap();
    assert!(json.get("frame_pacing").is_some() && json.get("buffers").is_some());
    let names: Vec<String> = crate::metrics::Metrics::metrics(&r)
        .into_iter()
        .map(|m| m.name)
        .collect();
    assert!(names.contains(&"frame_pacing.fps".to_string()));
    assert!(names.contains(&"buffers.distinct_buffers".to_string()));
    let section = both.section(&[&it], 0);
    assert_eq!(section.name, "frame_pacing+buffers");
}
