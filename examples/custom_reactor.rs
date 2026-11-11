//! A custom reactor: a jank watchdog that runs on the compositor thread. The reference client
//! animates a bar at 60 Hz, and every tenth frame renders 30 ms longer. The watchdog sees every
//! trace record as it happens. On each presentation of the window it computes the present
//! interval. An interval above 1.5 refresh periods is a long frame: the watchdog records a user
//! mark in the trace, sends the frame to the caller and moves the pointer at once. It then times
//! how long the client takes to handle that motion. A timer ends the watchdog after 3 seconds.
//! The client applies the motion in the next frame it starts, so that time is near zero when the
//! motion arrives before the frame callback, and near one refresh after it.
//!
//! The reactor reacts with zero round trip: its input goes out inside the dispatch that saw the
//! late frame. Every time is a compositor timestamp.
//!
//! Run with `just example custom_reactor`, or `cargo run -p fakeland-examples --example
//! custom_reactor` after `cargo build -p fakeland-testclient`.

use std::collections::HashMap;

use fakeland::scenario::{Position, Primitive};
use fakeland::{Event, Fakeland, LaunchSpec, OutputConfig, Reactor, ReactorCx, Record, WindowRef};
use fakeland_examples::testclient_argv;
use fakeland_trace::AppMarkKind;
use fakeland_types::{CommitId, Hz, InputEventId, Nanos, SurfaceId, Timestamp};

const RUN: Nanos = Nanos::from_secs(3);

/// A frame that presents late.
#[derive(Debug)]
struct LongFrame {
    /// The app frame id of the late commit.
    frame_id: Option<u64>,
    /// The commit that ends the long interval.
    commit: CommitId,
    /// The time from the previous presentation.
    interval: Nanos,
    /// The refresh period.
    period: Nanos,
    /// The client render span of the late commit.
    render: Option<Nanos>,
}

/// What the watchdog sends to the caller.
#[derive(Debug)]
enum Report {
    /// A long frame.
    Long(LongFrame),
    /// The client handled the pointer motion that followed a long frame.
    InputHandled { latency: Nanos },
    /// The end of the watch.
    Done { frames: u32, long: u32 },
}

/// Watches the presentations of one window for long frames.
struct JankWatchdog {
    window: WindowRef,
    surface: Option<SurfaceId>,
    last_present: Option<Timestamp>,
    /// The app frame id and render span of each commit that waits for presentation.
    pending: HashMap<CommitId, (Option<u64>, Option<Nanos>)>,
    /// The pointer motion after the last long frame, with its send time.
    probe: Option<(InputEventId, Timestamp)>,
    toggle: bool,
    frames: u32,
    long: u32,
}

impl JankWatchdog {
    fn new(window: WindowRef) -> Self {
        Self {
            window,
            surface: None,
            last_present: None,
            pending: HashMap::new(),
            probe: None,
            toggle: false,
            frames: 0,
            long: 0,
        }
    }

    /// Moves the pointer inside the window and remembers the first event.
    fn nudge(&mut self, cx: &mut ReactorCx<'_, Report>) {
        self.toggle = !self.toggle;
        let x = if self.toggle { 320.0 } else { 300.0 };
        let pos = Position::window(self.window.clone(), x, 400.0);
        match cx.inject(&[Primitive::PointerMotion { pos }, Primitive::PointerFrame]) {
            Ok(sent) => self.probe = sent.first().map(|s| (s.id, s.time)),
            Err(e) => eprintln!("pointer motion failed: {e}"),
        }
    }

    fn on_mark(&mut self, commit: CommitId, mark: &AppMarkKind, t: Timestamp) -> Option<Report> {
        let entry = self.pending.entry(commit).or_default();
        match mark {
            AppMarkKind::Frame { frame_id } => entry.0 = Some(*frame_id),
            AppMarkKind::Span { name, begin, end } if name == "render" => {
                entry.1 = Some(end.since(*begin));
            }
            AppMarkKind::InputHandled { input } if self.probe.is_some_and(|p| p.0 == *input) => {
                let (_, sent) = self.probe.take()?;
                return Some(Report::InputHandled {
                    latency: t.since(sent),
                });
            }
            _ => {}
        }
        None
    }

    fn on_present(
        &mut self,
        commit: CommitId,
        time: Timestamp,
        period: Nanos,
        cx: &mut ReactorCx<'_, Report>,
    ) {
        self.frames += 1;
        let (frame_id, render) = self.pending.remove(&commit).unwrap_or_default();
        self.pending.retain(|c, _| *c > commit);
        let last = self.last_present.replace(time);
        let Some(interval) = last.map(|l| time.since(l)) else {
            return;
        };
        if interval.as_nanos() * 2 <= period.as_nanos() * 3 {
            return;
        }
        self.long += 1;
        let id = frame_id.map_or("?".into(), |f| f.to_string());
        cx.mark(format!(
            "long frame {id}: {:.1} ms",
            interval.as_millis_f64()
        ));
        cx.emit(Report::Long(LongFrame {
            frame_id,
            commit,
            interval,
            period,
            render,
        }));
        self.nudge(cx);
    }
}

impl Reactor for JankWatchdog {
    type Value = Report;

    fn on_start(&mut self, cx: &mut ReactorCx<'_, Report>) {
        self.surface = cx.window(&self.window).map(|w| w.surface);
        if self.surface.is_none() {
            eprintln!("the window is not mapped");
            cx.finish();
            return;
        }
        let end = cx.now() + RUN;
        cx.wake_at(end, 0);
    }

    fn on_record(&mut self, record: &Record, cx: &mut ReactorCx<'_, Report>) {
        match &record.event {
            Event::AppMark {
                surface,
                commit: Some(commit),
                mark,
            } if *surface == self.surface => {
                if let Some(report) = self.on_mark(*commit, mark, record.t) {
                    cx.emit(report);
                }
            }
            Event::Presented {
                surface,
                commit,
                time,
                refresh,
                ..
            } if Some(*surface) == self.surface => {
                let Some(period) = refresh.or_else(|| cx.period(None)) else {
                    return;
                };
                self.on_present(*commit, *time, period, cx);
            }
            _ => {}
        }
    }

    fn on_timer(&mut self, _token: u64, cx: &mut ReactorCx<'_, Report>) {
        cx.emit(Report::Done {
            frames: self.frames,
            long: self.long,
        });
        cx.finish();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .app_marks(true)
        .start()?;
    let argv = testclient_argv(&[
        "--app-id",
        "example.jank",
        "--scene",
        "animate",
        "--pointer-marker",
        "--jank-every",
        "10",
        "--jank",
        "30ms",
    ]);
    let app = session.blocking_launch(LaunchSpec::from_argv(argv).ok_or("empty command")?)?;
    let me = WindowRef::app_id("example.jank");
    if let Err(e) = session.blocking_wait_window(me.clone(), Nanos::from_secs(10)) {
        return Err(format!("no window ({e}): {}", app.stderr()).into());
    }

    let handle = session.blocking_install_reactor(JankWatchdog::new(me))?;
    println!("watching for frames longer than 1.5 refresh periods");
    let mut intervals = Vec::new();
    let mut input = Vec::new();
    while let Some(report) = handle.blocking_recv() {
        match report {
            Report::Long(f) => {
                let missed = f.interval.as_nanos() as f64 / f.period.as_nanos() as f64 - 1.0;
                println!(
                    "frame {:>4} {:>11}: {:5.1} ms after the previous frame, missed {missed:.0} refresh(es), render {}",
                    f.frame_id.map_or("?".into(), |id| id.to_string()),
                    f.commit.to_string(),
                    f.interval.as_millis_f64(),
                    f.render
                        .map_or("?".into(), |r| format!("{:.1} ms", r.as_millis_f64())),
                );
                intervals.push(f.interval);
            }
            Report::InputHandled { latency } => input.push(latency),
            Report::Done { frames, long } => {
                println!(
                    "\n{frames} frames presented in {:.0} s, {long} long",
                    RUN.as_secs_f64()
                );
            }
        }
    }
    let summary = |v: &[Nanos]| {
        fakeland_trace::Summary::of_millis(v.iter().copied()).map_or("-".into(), |s| {
            format!("p50 {:.1} ms, max {:.1} ms", s.p50, s.max)
        })
    };
    println!("long frame interval: {}", summary(&intervals));
    println!(
        "pointer motion after a long frame, send -> handled commit: {} ({} samples)",
        summary(&input),
        input.len()
    );
    let trace = session.blocking_trace_snapshot()?;
    let marks = trace
        .records
        .iter()
        .filter(
            |r| matches!(&r.event, Event::UserMark { label } if label.starts_with("long frame")),
        )
        .count();
    println!("{marks} long-frame user marks in the trace");
    session.blocking_shutdown()?;
    Ok(())
}
