//! Checks of the app marks over the whole trace: frame ids, the order of the marks and the
//! commit, the input attribution, and the start-up marks.

use std::collections::BTreeMap;

use fakeland_trace::event::AppMarkKind;
use fakeland_trace::frames::{Attribution, FrameTimeline};
use fakeland_trace::{Event, Trace};
use fakeland_types::{Nanos, Timestamp};

use crate::breakdown::stats;

/// The result of the frame checks.
#[derive(Clone, Copy, Debug, Default)]
pub struct FrameChecks {
    /// Commits with a buffer.
    pub buffer_commits: usize,
    /// Buffer commits with an app frame id.
    pub with_frame_id: usize,
    /// Frame ids that do not increase in commit order.
    pub out_of_order: usize,
    /// Frames whose `frame` span ends after the commit.
    pub span_after_commit: usize,
}

impl FrameChecks {
    /// True when every buffer commit has a frame id in order and every frame span precedes its
    /// commit.
    pub fn ok(&self) -> bool {
        self.buffer_commits > 0
            && self.with_frame_id == self.buffer_commits
            && self.out_of_order == 0
            && self.span_after_commit == 0
    }
}

/// Checks the frame marks of every surface and prints the result.
pub fn frames(timeline: &FrameTimeline) -> FrameChecks {
    let mut c = FrameChecks::default();
    let mut lead = Vec::new();
    for surface in timeline.committed_surfaces() {
        let mut last = None;
        for f in timeline.frames(surface).iter().filter(|f| f.has_buffer()) {
            c.buffer_commits += 1;
            let Some(id) = f.marks.frame_id else {
                continue;
            };
            c.with_frame_id += 1;
            if last.is_some_and(|l| id <= l) {
                c.out_of_order += 1;
            }
            last = Some(id);
            if let Some(span) = f.marks.spans.iter().find(|s| s.name == "frame") {
                if span.end > f.committed {
                    c.span_after_commit += 1;
                }
                lead.push(Some(f.committed - span.end));
            }
        }
    }
    println!("\n== frame marks over the whole trace");
    println!(
        "  buffer commits {}  with app frame id {}  ids out of order {}  frame span after commit {}",
        c.buffer_commits, c.with_frame_id, c.out_of_order, c.span_after_commit
    );
    if let Some((p50, p90, n)) = stats(lead) {
        println!("  pre-present -> commit: p50 {p50:.3} ms  p90 {p90:.3} ms  over {n} frames");
    }
    c
}

/// Prints how the inputs of the trace map to commits, by kind and by attribution.
pub fn inputs(timeline: &FrameTimeline) {
    let mut by: BTreeMap<String, [usize; 3]> = BTreeMap::new();
    let mut handling = Vec::new();
    for i in timeline.inputs() {
        let slot = by.entry(format!("{:?}", i.kind)).or_default();
        match i.attribution {
            Attribution::Mark => {
                slot[0] += 1;
                handling.push(i.handling_latency());
            }
            Attribution::NextCommit => slot[1] += 1,
            Attribution::None => slot[2] += 1,
        }
    }
    println!("\n== input attribution (mark / next commit / none)");
    for (kind, [mark, next, none]) in &by {
        println!("  {kind:<16} {mark:>5} {next:>5} {none:>5}");
    }
    if let Some((p50, p90, n)) = stats(handling) {
        println!("  sent -> handling commit by mark: p50 {p50:.3} ms  p90 {p90:.3} ms  n {n}");
    }
}

/// Prints the start-up marks relative to the launch of the app, with the first presentation of a
/// buffer.
pub fn startup(trace: &Trace, timeline: &FrameTimeline) -> Vec<(String, Nanos)> {
    let spawned = trace.records.iter().find_map(|r| match &r.event {
        Event::ProcessSpawned { .. } => Some(r.t),
        _ => None,
    });
    let mut marks: Vec<(String, Timestamp)> = Vec::new();
    for r in &trace.records {
        if let Event::AppMark {
            mark: AppMarkKind::Mark { name },
            ..
        } = &r.event
            && !marks.iter().any(|(n, _)| n == name)
        {
            marks.push((name.clone(), r.t));
        }
    }
    let first_present = timeline
        .all_frames()
        .filter(|f| f.has_buffer())
        .filter_map(|f| f.presented_time())
        .min();
    println!("\n== start-up marks (from process spawn)");
    let Some(spawned) = spawned else {
        println!("  no process spawn in the trace");
        return Vec::new();
    };
    let mut out: Vec<(String, Nanos)> = marks.into_iter().map(|(n, t)| (n, t - spawned)).collect();
    if let Some(t) = first_present {
        out.push(("first buffer presented".into(), t - spawned));
    }
    for (name, at) in &out {
        println!("  {name:<28} {:>9.3} ms", at.as_millis_f64());
    }
    out
}
