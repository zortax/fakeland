//! Latency probes on real apps: hover on a GTK4 dialog button and key echo in a terminal, both
//! with pixel detection on the client buffers. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::scenario::{
    Action, Detector, KeyAction, PhaseSpec, PointerAction, Position, ProbeSpec, Roi,
};
use fakeland::{Event, Fakeland, LaunchSpec, OutputConfig, ProbeReport, Session, WindowRef};
use fakeland_trace::BufferKind;
use fakeland_types::{Dur, Hz, Nanos, Rect};

fn session() -> Session {
    Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1920, 1080, Hz(60.0)))
        .start()
        .expect("start")
}

fn print(report: &ProbeReport) {
    for r in &report.results {
        eprintln!(
            "  #{:02} phase {:>7.3} ms  input->commit {:>7.3}  commit->ready {:>6.3}  ready->latch {:>6.3}  latch->present {:>6.3}  total {:>7.3}  reads {} ({:.3} ms)",
            r.iteration,
            r.phase_in_frame.map_or(f64::NAN, |p| p.as_millis_f64()),
            r.stages
                .input_to_commit
                .map_or(f64::NAN, |n| n.as_millis_f64()),
            r.stages
                .commit_to_ready
                .map_or(f64::NAN, |n| n.as_millis_f64()),
            r.stages
                .ready_to_latch
                .map_or(f64::NAN, |n| n.as_millis_f64()),
            r.stages
                .latch_to_present
                .map_or(f64::NAN, |n| n.as_millis_f64()),
            r.stages.total.map_or(f64::NAN, |n| n.as_millis_f64()),
            r.readbacks,
            r.readback_cost.as_millis_f64(),
        );
    }
    let totals: Vec<i64> = report.totals().iter().map(|n| n.as_nanos()).collect();
    support::summarize(
        &format!("{} total", report.label.clone().unwrap_or_default()),
        &totals,
    );
    let i2c: Vec<i64> = report
        .input_to_commit()
        .iter()
        .map(|n| n.as_nanos())
        .collect();
    support::summarize(
        &format!("{} input->commit", report.label.clone().unwrap_or_default()),
        &i2c,
    );
}

#[test]
#[ignore = "runs zenity"]
fn gtk_button_hover_latency() {
    let _serial = support::serial();
    if !support::have("zenity") {
        eprintln!("zenity is missing");
        return;
    }
    let s = session();
    let app = s
        .blocking_launch(LaunchSpec::new("zenity").args(["--info", "--text", "fakeland probe"]))
        .expect("launch");
    let w = s
        .blocking_wait_window(WindowRef::Any, Nanos::from_secs(20))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let me = WindowRef::id(w.id);
    s.blocking_wait_settled(me.clone(), Nanos::from_millis(500), Nanos::from_secs(10))
        .expect("settled");
    let g = w.geometry;
    eprintln!("zenity window {}x{}", g.w, g.h);
    let (bx, by) = (g.w as f64 / 2.0, g.h as f64 - 48.0);
    let spec = ProbeSpec::new(
        Action::Pointer(PointerAction::move_to(Position::window(me.clone(), bx, by))),
        Detector::PixelsChanged {
            roi: Roi::window_rect(me.clone(), Rect::new(30, g.h - 66, g.w - 60, 36)),
            threshold: 0.01,
        },
    )
    .alternate(Action::Pointer(PointerAction::move_to(Position::window(
        me.clone(),
        bx,
        40.0,
    ))))
    .phase(PhaseSpec::Random { seed: 7 })
    .repeat(12)
    .label("zenity hover");
    let report = s.blocking_probe(spec).expect("probe");
    print(&report);
    assert!(report.timeouts() <= 1, "hover changes the button");
    let trace = s.blocking_trace_snapshot().expect("trace");
    let dmabuf = trace.records.iter().any(|r| {
        matches!(&r.event, Event::Commit { buffer: Some(b), .. } if b.kind == BufferKind::Dmabuf)
    });
    eprintln!("zenity draws with dmabuf buffers: {dmabuf}");
}

#[test]
#[ignore = "runs alacritty"]
fn terminal_key_echo_latency() {
    let _serial = support::serial();
    if !support::have("alacritty") {
        eprintln!("alacritty is missing");
        return;
    }
    let s = session();
    let app = s
        .blocking_launch(LaunchSpec::new("alacritty").args(["-e", "cat"]))
        .expect("launch");
    let w = s
        .blocking_wait_window(WindowRef::Any, Nanos::from_secs(20))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let me = WindowRef::id(w.id);
    s.blocking_wait_settled(me.clone(), Nanos::from_millis(500), Nanos::from_secs(10))
        .expect("settled");
    let spec = ProbeSpec::new(
        Action::Key(KeyAction::tap("x")),
        Detector::PixelsChanged {
            roi: Roi::window_rect(me.clone(), Rect::new(0, 0, 400, 120)),
            threshold: 0.0001,
        },
    )
    .alternate(Action::Key(KeyAction::tap("BackSpace")))
    .phase(PhaseSpec::Sweep { steps: 6 })
    .repeat(12)
    .label("alacritty echo");
    let mut spec = spec;
    spec.settle = Dur::from_millis(150);
    let report = s.blocking_probe(spec).expect("probe");
    print(&report);
    assert!(report.timeouts() <= 1, "the echo shows up");
    let trace = s.blocking_trace_snapshot().expect("trace");
    let kinds: Vec<BufferKind> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::Commit {
                buffer: Some(b), ..
            } => Some(b.kind),
            _ => None,
        })
        .collect();
    eprintln!(
        "alacritty buffers: {} dmabuf of {}",
        kinds.iter().filter(|k| **k == BufferKind::Dmabuf).count(),
        kinds.len()
    );
}
