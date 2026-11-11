//! Text tables of trace summaries and run validity.

use std::fmt::Write;

use fakeland::RunValidity;
use fakeland_trace::summary::SurfaceSummary;
use fakeland_trace::{Summary, TraceSummary};

fn ms(s: Option<&Summary>, f: impl Fn(&Summary) -> f64) -> String {
    s.map_or_else(|| "-".to_owned(), |s| format!("{:.2}", f(s)))
}

fn surface_name(s: &SurfaceSummary) -> String {
    let role = s.role.as_deref().unwrap_or("surface");
    match (&s.app_id, s.window) {
        (Some(app), Some(w)) if !app.is_empty() => format!("{app} ({w}, {role})"),
        (_, Some(w)) => format!("{w} {role}"),
        _ => format!("{} {role}", s.surface),
    }
}

/// Renders the outputs, the surfaces with presented or committed frames, and the input latency
/// of a trace summary.
pub fn summary_text(summary: &TraceSummary) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "trace: {:.1} ms, {} records{}",
        summary.duration_ms,
        summary.records,
        if summary.dropped_records > 0 {
            format!(", {} dropped", summary.dropped_records)
        } else {
            String::new()
        }
    );
    for o in &summary.outputs {
        let lateness = o.wake_lateness_us.as_ref().map_or_else(String::new, |l| {
            format!(", wake lateness p50 {:.0} us max {:.0} us", l.p50, l.max)
        });
        let _ = writeln!(
            out,
            "output {} {}: {} vblanks ({} dropped), {} presented{}",
            o.output, o.name, o.vblanks, o.dropped_vblanks, o.presented_frames, lateness
        );
    }
    let surfaces: Vec<&SurfaceSummary> =
        summary.surfaces.iter().filter(|s| s.commits > 0).collect();
    if surfaces.is_empty() {
        let _ = writeln!(out, "no surface committed a frame");
    }
    for s in surfaces {
        if s.presented < 2 {
            let _ = writeln!(
                out,
                "{}: commits {} presented {} discarded {} pending {}",
                surface_name(s),
                s.commits,
                s.presented,
                s.discarded,
                s.pending
            );
            continue;
        }
        let _ = writeln!(out, "{}", surface_name(s));
        let _ = writeln!(
            out,
            "  commits {} presented {} discarded {} pending {} missed vblanks {}",
            s.commits, s.presented, s.discarded, s.pending, s.vblanks_missed
        );
        if let Some(fps) = s.fps {
            let lows = s.lows.as_ref().map_or_else(String::new, |l| {
                format!(
                    "  1% low {:.1}  0.1% low {:.1}",
                    l.low_1pct_fps, l.low_01pct_fps
                )
            });
            let _ = writeln!(out, "  fps {fps:.2}{lows}");
        }
        let _ = writeln!(
            out,
            "  {:<18} {:>8} {:>8} {:>8} {:>8}",
            "ms", "p50", "p99", "max", "mean"
        );
        for (name, stat) in [
            ("present interval", &s.present_interval_ms),
            ("client", &s.client_time_ms),
            ("gpu", &s.gpu_time_ms),
            ("queue", &s.queue_time_ms),
            ("display latency", &s.display_latency_ms),
        ] {
            if stat.is_none() {
                continue;
            }
            let st = stat.as_ref();
            let _ = writeln!(
                out,
                "  {:<18} {:>8} {:>8} {:>8} {:>8}",
                name,
                ms(st, |s| s.p50),
                ms(st, |s| s.p99),
                ms(st, |s| s.max),
                ms(st, |s| s.mean)
            );
        }
        if !s.formats.is_empty() {
            let _ = writeln!(
                out,
                "  buffers {} formats {} sync none {} implicit {} explicit {}",
                s.buffers,
                s.formats.join(","),
                s.sync.none,
                s.sync.implicit,
                s.sync.explicit
            );
        }
    }
    let i = &summary.inputs;
    if i.count > 0 {
        let lat = ms(i.latency_ms.as_ref(), |s| s.p50);
        let handling = ms(i.handling_ms.as_ref(), |s| s.p50);
        let _ = writeln!(
            out,
            "input: {} events, input to present p50 {lat} ms, input to commit p50 {handling} ms",
            i.count
        );
    }
    out
}

/// Renders the compositor lateness of a run.
pub fn validity_text(v: &RunValidity) -> String {
    format!(
        "compositor: {} wakes, {} late, max lateness {:.3} ms (threshold {:.3} ms): {}",
        v.wakes,
        v.late_wakes,
        v.max_lateness.as_millis_f64(),
        v.threshold.as_millis_f64(),
        if v.is_valid() { "valid" } else { "INVALID" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_trace::{Environment, Event, FrameTimeline, Trace, TraceHeader};
    use fakeland_types::Timestamp;

    #[test]
    fn renders_an_empty_trace() {
        let mut trace = Trace::new(TraceHeader::new(
            Environment::default(),
            Vec::new(),
            Timestamp(0),
        ));
        trace.push(Timestamp(0), Event::UserMark { label: "x".into() });
        let summary = TraceSummary::build(&trace, &FrameTimeline::build(&trace));
        let text = summary_text(&summary);
        assert!(text.starts_with("trace: 0.0 ms, 1 records"));
        assert!(text.contains("no surface committed a frame"));
    }
}
