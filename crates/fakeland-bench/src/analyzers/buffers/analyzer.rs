//! The buffers analyzer.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use fakeland_trace::{BufferInfo, FrameRecord, Summary};
use fakeland_types::{BufferId, Nanos};

use super::{BuffersReport, FormatCount, ScaleFit};
use crate::analyzer::Analyzer;
use crate::analyzers::util::{logical_size, physical_size_matches};
use crate::input::AnalysisInput;

/// Measures the buffer use of the target window tree.
#[derive(Clone, Debug, Default)]
pub struct Buffers;

impl Analyzer for Buffers {
    type Report = BuffersReport;

    fn name(&self) -> String {
        "buffers".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> BuffersReport {
        let tl = &input.timeline;
        let mut frames: Vec<(&FrameRecord, &BufferInfo)> = input
            .target_surfaces()
            .into_iter()
            .flat_map(|s| tl.frames(s).iter())
            .filter_map(|f| Some((f, f.buffer.as_ref()?)))
            .collect();
        frames.sort_by_key(|(f, _)| (f.committed, f.commit));
        let mut first_seen: HashMap<BufferId, fakeland_types::Timestamp> = HashMap::new();
        let mut kinds = BTreeMap::new();
        let mut sync = BTreeMap::new();
        let mut formats: BTreeMap<(String, Option<u64>), usize> = BTreeMap::new();
        let mut sizes = BTreeSet::new();
        let mut scale = ScaleFit::default();
        let mut in_flight = Vec::with_capacity(frames.len());
        for (i, (f, b)) in frames.iter().enumerate() {
            first_seen.entry(b.id).or_insert(f.committed);
            *kinds.entry(b.kind.as_str().to_string()).or_insert(0) += 1;
            *sync.entry(f.sync.as_str().to_string()).or_insert(0) += 1;
            *formats.entry((b.fourcc(), b.modifier)).or_insert(0) += 1;
            sizes.insert((b.width, b.height));
            let busy = frames[..i]
                .iter()
                .filter(|(e, _)| e.released.is_none_or(|r| r > f.committed))
                .map(|(_, b)| b.id)
                .collect::<BTreeSet<_>>()
                .len()
                + 1;
            in_flight.push(busy as f64);
            if let Some(p) = f.presented {
                let output_scale = tl.output(p.output).map_or(1.0, |o| o.scale);
                let (lw, lh) = logical_size(b);
                if physical_size_matches(b, output_scale) {
                    scale.native += 1;
                } else if f64::from(b.width) * f64::from(b.height)
                    < lw * lh * output_scale * output_scale
                {
                    scale.upscaled += 1;
                } else {
                    scale.oversampled += 1;
                }
            }
        }
        let start = frames.first().map(|(f, _)| f.committed);
        let end = frames.last().map(|(f, _)| f.committed);
        let after = start.map(|s| s + Nanos::from_secs(1));
        let late = after.map_or(0, |a| first_seen.values().filter(|t| **t > a).count());
        let churn_span = after.zip(end).map(|(a, e)| e.since(a).as_secs_f64());
        BuffersReport {
            buffer_commits: frames.len(),
            distinct_buffers: first_seen.len(),
            distinct_sizes: sizes.len(),
            max_in_flight: in_flight.iter().copied().fold(0.0, f64::max) as usize,
            in_flight: Summary::of(&in_flight),
            late_allocations: late,
            churn_per_s: churn_span.filter(|s| *s > 0.0).map(|s| late as f64 / s),
            kinds,
            formats: formats
                .into_iter()
                .map(|((fourcc, modifier), count)| FormatCount {
                    fourcc,
                    modifier,
                    count,
                })
                .collect(),
            sync,
            scale,
        }
    }
}
