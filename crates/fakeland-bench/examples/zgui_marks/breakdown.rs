//! The stage breakdown of each probe repeat and the agreement of the two detectors.
//!
//! The app marks name the commit that handled a probe input. That commit carries the app frame id
//! and the app spans, so one repeat splits into: input sent, dispatched by the app, frame begin,
//! the frame phases, the commit, the buffer ready, the latch and the presentation.

use fakeland::ProbeResult;
use fakeland_trace::frames::{Attribution, FrameRecord, FrameTimeline};
use fakeland_types::{CommitId, InputEventId, Nanos, Timestamp};

use crate::probes::{Kind, ProbeRun};

/// The app phases in pipeline order, as zgui names its spans.
pub const PHASES: &[&str] = &[
    "events", "update", "style", "layout", "paint", "record", "held", "acquire", "submit",
];

/// The stage rows of a breakdown, in order.
pub const STAGES: &[&str] = &[
    "sent -> app dispatch",
    "app dispatch -> frame begin",
    "frame (begin -> pre-present)",
    "pre-present -> commit",
    "commit -> ready",
    "ready -> latch",
    "latch -> present",
    "sent -> commit",
    "total (sent -> present)",
];

/// One probe repeat split into stages.
#[derive(Clone, Debug)]
pub struct Repeat {
    /// The repeat index.
    pub iteration: u32,
    /// The first probe input.
    pub input: Option<InputEventId>,
    /// The commit the probe detected.
    pub detected: Option<CommitId>,
    /// The commit that the app marks as handling a probe input.
    pub handled: Option<CommitId>,
    /// The app frame id of the handling commit.
    pub frame_id: Option<u64>,
    /// The stage values, in the order of `STAGES`.
    pub stages: Vec<Option<Nanos>>,
    /// The app phase values, in the order of `PHASES`.
    pub phases: Vec<Option<Nanos>>,
}

impl Repeat {
    /// True when both commits exist and are the same.
    pub fn agrees(&self) -> bool {
        self.detected.is_some() && self.detected == self.handled
    }
}

/// Splits each repeat of a probe run.
pub fn repeats(run: &ProbeRun, timeline: &FrameTimeline) -> Vec<Repeat> {
    run.report
        .results
        .iter()
        .map(|r| repeat(r, timeline))
        .collect()
}

fn repeat(r: &ProbeResult, timeline: &FrameTimeline) -> Repeat {
    // The earliest commit that the marks name for any input of the repeat.
    let handled = r
        .inputs
        .iter()
        .filter_map(|id| timeline.input(*id))
        .filter(|i| i.attribution == Attribution::Mark)
        .filter_map(|i| i.handled_by.map(|c| (i, c)))
        .min_by_key(|(i, _)| i.handled_at);
    let sent = r.input_sent;
    let frame = handled.and_then(|(_, c)| timeline.frame(c));
    let span = |name: &str| frame.and_then(|f| f.marks.spans.iter().find(|s| s.name == name));
    let input_wait = span("input_wait");
    let whole = span("frame");
    let committed = frame.map(|f| f.committed);
    // The probe's own stage times are model times. They apply when both detectors name the
    // same commit; otherwise the trace record of the handling commit gives them.
    let same = r.detected_commit.is_some() && r.detected_commit == handled.map(|(_, c)| c);
    let ready = pick(same, r.ready, frame.map(FrameRecord::effective_ready));
    let latched = pick(same, r.latched, frame.and_then(|f| f.latched.map(|l| l.t)));
    let presented = pick(same, r.presented, handled.and_then(|(i, _)| i.presented));
    let stages = vec![
        diff(sent, input_wait.map(|s| s.begin)),
        input_wait.map(|s| s.duration()),
        whole.map(|s| s.duration()),
        diff(whole.map(|s| s.end), committed),
        diff(committed, ready),
        diff(ready, latched),
        diff(latched, presented),
        diff(sent, committed),
        diff(sent, presented),
    ];
    let phases = PHASES
        .iter()
        .map(|name| span(name).map(|s| s.duration()))
        .collect();
    Repeat {
        iteration: r.iteration,
        input: r.inputs.first().copied(),
        detected: r.detected_commit,
        handled: handled.map(|(_, c)| c),
        frame_id: frame.and_then(|f| f.marks.frame_id),
        stages,
        phases,
    }
}

fn pick(same: bool, probe: Option<Timestamp>, trace: Option<Timestamp>) -> Option<Timestamp> {
    if same { probe.or(trace) } else { trace }
}

fn diff(a: Option<Timestamp>, b: Option<Timestamp>) -> Option<Nanos> {
    Some(b? - a?)
}

/// The median and the 90th percentile in milliseconds, with the sample count.
pub fn stats(values: impl IntoIterator<Item = Option<Nanos>>) -> Option<(f64, f64, usize)> {
    let mut v: Vec<f64> = values
        .into_iter()
        .flatten()
        .map(Nanos::as_millis_f64)
        .collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let at = |q: f64| v[((v.len() - 1) as f64 * q).round() as usize];
    Some((at(0.5), at(0.9), v.len()))
}

/// Prints the breakdown table of a probe run and returns how many repeats agree.
pub fn print(run: &ProbeRun, list: &[Repeat], verbose: bool) -> (usize, usize) {
    let detected = run
        .report
        .results
        .iter()
        .filter(|r| r.detected_commit.is_some())
        .count();
    let handled = list.iter().filter(|r| r.handled.is_some()).count();
    let agree = list.iter().filter(|r| r.agrees()).count();
    println!(
        "\n== {}  repeats {}  detected {}  marked {}  detector == marked commit {}/{}",
        run.label(),
        list.len(),
        detected,
        handled,
        agree,
        list.len()
    );
    println!(
        "  {:<34} {:>9} {:>9} {:>4}",
        "stage", "p50 ms", "p90 ms", "n"
    );
    for (i, name) in STAGES.iter().enumerate() {
        row(name, stats(list.iter().map(|r| r.stages[i])));
        if *name == "frame (begin -> pre-present)" {
            for (j, phase) in PHASES.iter().enumerate() {
                row(
                    &format!("  app span {phase}"),
                    stats(list.iter().map(|r| r.phases[j])),
                );
            }
        }
    }
    // The detector's own input -> commit, independent of the app marks.
    row(
        "detector: sent -> detected commit",
        stats(run.report.results.iter().map(|r| r.stages.input_to_commit)),
    );
    if verbose {
        for r in list {
            println!(
                "  #{:<3} input {:<6} detected {:<8} marked {:<8} frame {:<5} total {}",
                r.iteration,
                r.input.map_or("-".into(), |i| i.0.to_string()),
                r.detected.map_or("-".into(), |c| c.0.to_string()),
                r.handled.map_or("-".into(), |c| c.0.to_string()),
                r.frame_id.map_or("-".into(), |f| f.to_string()),
                ms(r.stages[8]),
            );
        }
    }
    if run.kind == Kind::Pixels {
        for r in list.iter().filter(|r| !r.agrees()) {
            println!(
                "  disagreement: repeat {} pixels commit {:?} marked commit {:?}",
                r.iteration, r.detected, r.handled
            );
        }
    }
    (agree, list.len())
}

fn row(name: &str, s: Option<(f64, f64, usize)>) {
    match s {
        Some((p50, p90, n)) => println!("  {name:<34} {p50:>9.3} {p90:>9.3} {n:>4}"),
        None => println!("  {name:<34} {:>9} {:>9} {:>4}", "-", "-", 0),
    }
}

fn ms(v: Option<Nanos>) -> String {
    v.map_or("-".into(), |n| format!("{:.3} ms", n.as_millis_f64()))
}
