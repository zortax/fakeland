//! Text form of a run summary.

use std::fmt::Write;

use crate::methods::RunSummary;

/// Renders the outcome, steps, probes, triggers and files of a run.
pub fn run_text(s: &RunSummary) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "run {} {:?}: {} in {:.1} ms",
        s.run.0,
        s.name,
        if s.ok { "ok" } else { "FAILED" },
        s.duration_ms
    );
    if let Some(e) = &s.error {
        let _ = writeln!(out, "  error: {e}");
    }
    for step in &s.steps {
        let _ = writeln!(
            out,
            "  {} {} at +{:.2} ms, {:.2} ms{}{}",
            step.path,
            step.action,
            step.started.since(s.started).as_millis_f64(),
            step.finished.since(step.started).as_millis_f64(),
            if step.inputs > 0 {
                format!(", {} inputs", step.inputs)
            } else {
                String::new()
            },
            step.label
                .as_deref()
                .map_or_else(String::new, |l| format!(" ({l})"))
        );
    }
    for p in &s.probes {
        let totals: Vec<f64> = p
            .results
            .iter()
            .filter_map(|r| r.stages.total.map(|t| t.as_millis_f64()))
            .collect();
        let median = median(&totals);
        let _ = writeln!(
            out,
            "  probe {} ({}): {} results, {} timeouts, total p50 {}",
            p.label.as_deref().unwrap_or("-"),
            p.detector,
            p.results.len(),
            p.results.iter().filter(|r| r.timed_out).count(),
            median.map_or_else(|| "-".to_owned(), |m| format!("{m:.2} ms"))
        );
    }
    if !s.triggers.is_empty() {
        let _ = writeln!(out, "  {} trigger firings", s.triggers.len());
    }
    for c in &s.captures {
        if let Some(p) = &c.shot.path {
            let _ = writeln!(out, "  capture {} -> {}", c.step, p.display());
        }
    }
    if let Some(t) = &s.trace
        && let Some(p) = &t.path
    {
        let _ = writeln!(out, "  trace -> {}", p.display());
    }
    for e in &s.file_errors {
        let _ = writeln!(out, "  file error: {e}");
    }
    out
}

fn median(values: &[f64]) -> Option<f64> {
    fakeland_trace::stats::median(values)
}
