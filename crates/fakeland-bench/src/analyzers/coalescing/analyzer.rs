//! The input coalescing analyzer.

use std::collections::{BTreeMap, HashMap};

use fakeland_trace::Summary;
use fakeland_types::CommitId;

use super::CoalescingReport;
use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;

/// Counts injected input events per presented frame of the target.
#[derive(Clone, Debug, Default)]
pub struct InputCoalescing;

impl Analyzer for InputCoalescing {
    type Report = CoalescingReport;

    fn name(&self) -> String {
        "input_coalescing".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> CoalescingReport {
        let surfaces = input.target_surfaces();
        let tl = &input.timeline;
        let inputs: Vec<_> = tl
            .inputs()
            .iter()
            .filter(|i| i.kind.is_actionable())
            .filter(|i| i.surface.is_none_or(|s| surfaces.contains(&s)))
            .collect();
        let mut per: HashMap<CommitId, usize> = HashMap::new();
        let mut by_kind = BTreeMap::new();
        for i in &inputs {
            *by_kind.entry(i.kind.as_str().to_string()).or_insert(0) += 1;
            if let Some(c) = i.presented_commit {
                *per.entry(c).or_insert(0) += 1;
            }
        }
        let counts: Vec<f64> = per.values().map(|n| *n as f64).collect();
        let span = match (inputs.first(), inputs.last()) {
            (Some(a), Some(b)) if inputs.len() > 1 => Some(b.sent.since(a.sent).as_secs_f64()),
            _ => None,
        }
        .filter(|s| *s > 0.0);
        CoalescingReport {
            inputs: inputs.len(),
            presented_inputs: per.values().sum(),
            frames: per.len(),
            per_frame: Summary::of(&counts),
            max_per_frame: per.values().copied().max().unwrap_or(0),
            input_rate_hz: span.map(|s| (inputs.len() - 1) as f64 / s),
            frame_rate_hz: span.map(|s| per.len() as f64 / s),
            by_kind,
        }
    }
}
