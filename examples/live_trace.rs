//! Traces. The example starts a trace capture and a live subscription, runs an animating client
//! while a scenario circles the pointer over its window, and prints a ticker once per second of
//! compositor time from the live records: frames presented, the mean present interval and the
//! input events sent. Then it ends the capture and exports it as Perfetto (Chrome JSON), a
//! per-frame CSV table, JSONL and a JSON summary.
//!
//! Run with `just example live_trace`. It runs the reference client; arguments after `--` run
//! another app, for example `-- vkcube --wsi wayland`. The exports go to
//! `target/examples/live_trace.*`.

use std::fs::File;
use std::io::BufWriter;

use fakeland::scenario::{Path, PointerGlide, Position, Scenario};
use fakeland::{Event, Fakeland, LaunchSpec, OutputConfig, SubscriptionItem, WindowRef};
use fakeland_examples::{app_command, init_logging, out_dir, testclient_argv};
use fakeland_trace::{FrameTimeline, TraceSummary, csv, write_chrome_trace};
use fakeland_types::{Dur, Hz, Nanos, SurfaceId, Timestamp};

type Error = Box<dyn std::error::Error>;

/// The length of the live part in seconds.
const SECONDS: i64 = 4;

/// The counts of one ticker second.
#[derive(Default)]
struct Tick {
    frames: u32,
    interval_sum: Nanos,
    intervals: u32,
    inputs: u32,
    vblanks: u32,
    missed: u64,
}

impl Tick {
    fn print(&self, second: i64) {
        let mean = if self.intervals > 0 {
            format!(
                "{:6.2} ms",
                self.interval_sum.as_millis_f64() / f64::from(self.intervals)
            )
        } else {
            "     - ms".to_owned()
        };
        let missed = if self.missed > 0 {
            format!(", {} records lost", self.missed)
        } else {
            String::new()
        };
        println!(
            "  t={second}s  {:4} frames  mean interval {mean}  {:4} inputs  {:4} vblanks{missed}",
            self.frames, self.inputs, self.vblanks
        );
    }
}

/// Reads live records until `SECONDS` of compositor time pass and prints a line per second.
fn ticker(sub: &mut fakeland::Subscription, surface: SurfaceId) {
    let mut start: Option<Timestamp> = None;
    let mut second = 0;
    let mut tick = Tick::default();
    let mut last_present: Option<Timestamp> = None;
    while let Some(item) = sub.blocking_next() {
        let record = match item {
            SubscriptionItem::Record(r) => r,
            SubscriptionItem::Gap { missed } => {
                tick.missed += missed;
                continue;
            }
        };
        let start = *start.get_or_insert(record.t);
        while record.t.since(start) >= Nanos::from_secs(second + 1) {
            tick.print(second);
            tick = Tick::default();
            second += 1;
        }
        if second >= SECONDS {
            break;
        }
        match record.event {
            Event::Presented {
                surface: s, time, ..
            } if s == surface => {
                tick.frames += 1;
                if let Some(prev) = last_present {
                    tick.interval_sum += time.since(prev);
                    tick.intervals += 1;
                }
                last_present = Some(time);
            }
            Event::InputSent { .. } => tick.inputs += 1,
            Event::Vblank { .. } => tick.vblanks += 1,
            _ => {}
        }
    }
}

fn main() -> Result<(), Error> {
    init_logging();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .app_marks(true)
        .start()?;
    let argv = app_command()
        .unwrap_or_else(|| testclient_argv(&["--scene", "animate,hover", "--pointer-marker"]));
    let app = session.blocking_launch(LaunchSpec::from_argv(argv).ok_or("empty command")?)?;
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .map_err(|e| format!("no window ({e}): {}", app.stderr()))?;
    let g = window.geometry;
    println!("{:?} maps at {}x{}+{}+{}", window.title, g.w, g.h, g.x, g.y);

    let capture = session.blocking_begin_capture()?;
    let mut sub = session.blocking_subscribe(4096)?;
    let me = WindowRef::id(window.id);
    // The radius has the units of the center frame, so the center is in window coordinates.
    let (w, h) = (f64::from(window.geometry.w), f64::from(window.geometry.h));
    let circle = Path::circle(Position::window(me, w / 2.0, h / 2.0), w.min(h) / 4.0, 3.0);
    let scenario = Scenario::builder("circle")
        .at_next_vblank()
        .glide(PointerGlide::new(
            circle,
            Dur::from_millis(SECONDS * 1000 - 500),
        ))
        .build()?;
    let run = session.blocking_run(scenario)?;
    println!("live ticker:");
    ticker(&mut sub, window.surface);
    drop(sub);
    let result = run.blocking_wait()?;
    let trace = session.blocking_end_capture(capture)?;
    session.blocking_shutdown()?;
    println!("scenario ok: {}", result.ok);

    let timeline = FrameTimeline::build(&trace);
    let summary = TraceSummary::build(&trace, &timeline);
    let dir = out_dir();
    let perfetto = dir.join("live_trace.perfetto.json");
    write_chrome_trace(&trace, &timeline, BufWriter::new(File::create(&perfetto)?))?;
    let frames = dir.join("live_trace.frames.csv");
    csv::write_frames(&timeline, BufWriter::new(File::create(&frames)?))?;
    let jsonl = dir.join("live_trace.jsonl");
    trace.write_jsonl(BufWriter::new(File::create(&jsonl)?))?;
    let summary_path = dir.join("live_trace.summary.json");
    summary.write_json(BufWriter::new(File::create(&summary_path)?))?;

    println!(
        "captured {} records over {:.0} ms, {} input events",
        summary.records, summary.duration_ms, summary.inputs.count
    );
    if let Some(s) = summary
        .surfaces
        .iter()
        .find(|s| s.surface == window.surface)
    {
        let interval = s.present_interval_ms.map_or(f64::NAN, |i| i.p50);
        println!(
            "window: {} commits, {} presented, {:.1} fps, p50 interval {interval:.2} ms",
            s.commits,
            s.presented,
            s.fps.unwrap_or(f64::NAN)
        );
    }
    if let Some(l) = summary.inputs.latency_ms {
        println!("input to photon: p50 {:.2} ms, p99 {:.2} ms", l.p50, l.p99);
    }
    println!("perfetto: {}", perfetto.display());
    println!("frames:   {}", frames.display());
    println!("jsonl:    {}", jsonl.display());
    println!("summary:  {}", summary_path.display());
    println!("open the perfetto file at https://ui.perfetto.dev (Open trace file)");
    Ok(())
}
