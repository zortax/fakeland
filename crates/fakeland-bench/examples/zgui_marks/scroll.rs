//! The scroll glide: a finger glide in one direction, a capture of every presented frame, and the
//! comparison of the scroll that the app marks attribute to each frame with the scroll that the
//! pixels show.

use fakeland::scenario::{Position, Scenario, ScrollGlide, ScrollSource};
use fakeland::{CaptureSpec, RunOptions, Session, WindowInfo};
use fakeland_bench::analyzers::ScrollSmoothness;
use fakeland_bench::analyzers::scroll::ScrollReport;
use fakeland_bench::{AnalysisInput, Analyzer, BenchError, Target};
use fakeland_types::{Dur, Nanos};

use crate::args::Scroll;
use crate::breakdown::stats;

/// The scroll result.
#[derive(Clone, Debug)]
pub struct ScrollOutcome {
    /// The direction: down or up.
    pub direction: &'static str,
    /// The smoothness report.
    pub report: ScrollReport,
    /// Steps where marks and pixels both saw motion and agree within one pixel.
    pub agree: usize,
    /// Steps where marks and pixels both saw motion.
    pub moving: usize,
    /// Steps with pixel motion and no marked scroll input.
    pub pixels_only: usize,
    /// Steps with a marked scroll input and no pixel motion.
    pub marks_only: usize,
}

/// Runs one glide, down for a positive `sign` and up for a negative one, and analyzes it.
pub fn run(
    session: &Session,
    window: &WindowInfo,
    scroll: Scroll,
    sign: f64,
) -> Result<ScrollOutcome, BenchError> {
    let at = Position::window(window.id, scroll.at.x, scroll.at.y);
    let glide = ScrollGlide::constant(
        sign * scroll.velocity,
        Dur::from_millis(scroll.millis as i64),
    )
    .at(at)
    .source(ScrollSource::finger(false));
    let scenario = Scenario::builder("zgui scroll glide")
        .scroll_glide(glide)
        .sleep(Dur::from_millis(400))
        .build()?;
    let capture = CaptureSpec::every_presented(window.id).with_roi(scroll.roi);
    let result = session
        .blocking_run_with(
            scenario,
            RunOptions {
                frames: vec![capture],
            },
        )?
        .blocking_wait()?;
    if !result.ok {
        return Err(BenchError::Driver(format!(
            "the glide failed: {:?}",
            result.error
        )));
    }
    let input = AnalysisInput::new(result.trace.clone())
        .with_target(Some(Target::from(window)))
        .with_frames(result.frames.clone());
    let report = ScrollSmoothness::default().analyze(&input);
    let mut outcome = ScrollOutcome {
        direction: if sign > 0.0 { "down" } else { "up" },
        report,
        agree: 0,
        moving: 0,
        pixels_only: 0,
        marks_only: 0,
    };
    for step in &outcome.report.steps {
        let Some(measured) = step.measured_px else {
            continue;
        };
        let marked = step.expected_px.abs() >= 0.5;
        let moved = measured.abs() >= 0.5;
        match (marked, moved) {
            (true, true) => {
                outcome.moving += 1;
                if (measured - step.expected_px).abs() <= 1.0 {
                    outcome.agree += 1;
                }
            }
            (false, true) => outcome.pixels_only += 1,
            (true, false) => outcome.marks_only += 1,
            (false, false) => {}
        }
    }
    Ok(outcome)
}

/// Prints the scroll result.
pub fn print(o: &ScrollOutcome, verbose: bool) {
    let r = &o.report;
    println!("\n== scroll glide {} (finger)", o.direction);
    println!(
        "  presented frames {}  axis inputs {}  input px {:.1}  marked px {:.1}  measured px {:.1}",
        r.frames, r.inputs, r.input_total_px, r.expected_total_px, r.measured_total_px
    );
    println!(
        "  tracking ratio {}  relative error {}  smoothness {}  start latency {}",
        opt(r.tracking_ratio, ""),
        opt(r.relative_error.map(|e| e * 100.0), "%"),
        opt(r.smoothness_score, ""),
        opt(r.start_latency_ms, " ms"),
    );
    println!(
        "  missed vblanks {}  stalled frames {}  double steps {}  rejected steps {}",
        r.missed_vblanks, r.stalled_frames, r.double_steps, r.rejected_steps
    );
    println!(
        "  marked and measured motion agree within 1 px: {}/{} steps; pixel-only {}, marks-only {}",
        o.agree, o.moving, o.pixels_only, o.marks_only
    );
    let intervals = r
        .steps
        .iter()
        .filter(|s| s.active)
        .map(|s| Some(Nanos::from_secs_f64(s.interval_ms / 1000.0)));
    if let Some((p50, p90, n)) = stats(intervals) {
        println!("  active present interval p50 {p50:.3} ms  p90 {p90:.3} ms  n {n}");
    }
    if verbose {
        for s in &r.steps {
            println!(
                "  commit {:<6} interval {:>7.3} ms  marked {:>7.2} px  measured {:>8}",
                s.commit.0,
                s.interval_ms,
                s.expected_px,
                s.measured_px.map_or("-".into(), |m| format!("{m:.2} px")),
            );
        }
    }
}

fn opt(v: Option<f64>, unit: &str) -> String {
    v.map_or("-".into(), |v| format!("{v:.3}{unit}"))
}
