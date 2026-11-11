//! Text form of a probe report.

use std::fmt::Write;

use fakeland::ProbeReport;
use fakeland_trace::Summary;
use fakeland_types::Nanos;

fn ms(v: Option<Nanos>) -> String {
    v.map_or_else(|| "-".to_owned(), |n| format!("{:.3}", n.as_millis_f64()))
}

/// Renders a probe report as a table of stages per repeat.
pub fn probe_text(r: &ProbeReport) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "probe {} ({}): {} results",
        r.label.as_deref().unwrap_or("-"),
        r.detector,
        r.results.len()
    );
    let _ = writeln!(
        out,
        "{:>4} {:>12} {:>12} {:>12} {:>12} {:>10}",
        "iter", "input>commit", "commit>ready", "ready>latch", "latch>pres", "total ms"
    );
    for res in &r.results {
        if res.timed_out {
            let _ = writeln!(out, "{:>4} timed out", res.iteration);
            continue;
        }
        let s = &res.stages;
        let _ = writeln!(
            out,
            "{:>4} {:>12} {:>12} {:>12} {:>12} {:>10}",
            res.iteration,
            ms(s.input_to_commit),
            ms(s.commit_to_ready),
            ms(s.ready_to_latch),
            ms(s.latch_to_present),
            ms(s.total)
        );
    }
    let totals: Vec<f64> = r
        .results
        .iter()
        .filter_map(|res| res.stages.total.map(Nanos::as_millis_f64))
        .collect();
    let commits: Vec<f64> = r
        .results
        .iter()
        .filter_map(|res| res.stages.input_to_commit.map(Nanos::as_millis_f64))
        .collect();
    if let (Some(t), Some(c)) = (Summary::of(&totals), Summary::of(&commits)) {
        let _ = writeln!(
            out,
            "input to present: p50 {:.2} ms, p95 {:.2} ms, max {:.2} ms; input to commit: p50 {:.2} ms; {} of {} detected",
            t.p50,
            t.p95,
            t.max,
            c.p50,
            totals.len(),
            r.results.len()
        );
    }
    out
}
