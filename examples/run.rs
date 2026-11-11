//! Runs a command in a headless session for a while and prints a trace summary.
//!
//! Usage: `cargo run -p fakeland-examples --example run -- [--hz N] [--scale S] [--size WxH]
//! [--secs T] [--pixman] [--unthrottled] [--shot out.png] [--trace out.jsonl] -- <command>
//! [args...]`

use std::fs::File;
use std::io::BufWriter;
use std::time::Duration;

use fakeland::{
    CaptureOptions, CaptureTarget, Fakeland, LaunchSpec, OutputConfig, RendererChoice,
    TimingConfig, WindowRef,
};
use fakeland_trace::{FrameTimeline, TraceSummary};
use fakeland_types::{Hz, Nanos};

struct Args {
    hz: f64,
    scale: f64,
    size: (i32, i32),
    secs: f64,
    pixman: bool,
    unthrottled: bool,
    shot: Option<String>,
    trace: Option<String>,
    command: Vec<String>,
}

fn parse() -> Result<Args, String> {
    let mut args = Args {
        hz: 60.0,
        scale: 1.0,
        size: (1920, 1080),
        secs: 3.0,
        pixman: false,
        unthrottled: false,
        shot: None,
        trace: None,
        command: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--hz" => args.hz = value()?.parse().map_err(|e| format!("--hz: {e}"))?,
            "--scale" => args.scale = value()?.parse().map_err(|e| format!("--scale: {e}"))?,
            "--secs" => args.secs = value()?.parse().map_err(|e| format!("--secs: {e}"))?,
            "--size" => {
                let v = value()?;
                let (w, h) = v.split_once('x').ok_or("--size takes WxH")?;
                args.size = (
                    w.parse().map_err(|e| format!("--size: {e}"))?,
                    h.parse().map_err(|e| format!("--size: {e}"))?,
                );
            }
            "--pixman" => args.pixman = true,
            "--unthrottled" => args.unthrottled = true,
            "--shot" => args.shot = Some(value()?),
            "--trace" => args.trace = Some(value()?),
            "--" => {
                args.command = it.by_ref().collect();
                break;
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if args.command.is_empty() {
        return Err("no command given".into());
    }
    Ok(args)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let args = parse()?;
    let mut output = OutputConfig::new("DP-1")
        .mode(args.size.0, args.size.1, Hz(args.hz))
        .scale(args.scale);
    if args.unthrottled {
        output = output.timing(TimingConfig::unthrottled());
    }
    let session = Fakeland::builder()
        .output(output)
        .renderer(if args.pixman {
            RendererChoice::Pixman
        } else {
            RendererChoice::Auto
        })
        .start()?;
    eprintln!("socket {}", session.socket_name());
    let capture = session.blocking_begin_capture()?;
    let spec = LaunchSpec::from_argv(args.command.clone()).ok_or("empty command")?;
    let app = session.blocking_launch(spec)?;
    match app.blocking_wait_window(Nanos::from_secs(10)) {
        Ok(w) => eprintln!("window {} {:?} {:?}", w.id, w.app_id, w.title),
        Err(e) => eprintln!("no window: {e}"),
    }
    std::thread::sleep(Duration::from_secs_f64(args.secs));
    if let Some(path) = &args.shot {
        let shot = session.blocking_screenshot(
            CaptureTarget::Window {
                window: WindowRef::Any,
            },
            CaptureOptions::default(),
        );
        let shot = match shot {
            Ok(s) => s,
            Err(_) => {
                session.blocking_screenshot(CaptureTarget::default(), CaptureOptions::default())?
            }
        };
        std::fs::write(path, shot.image.encode_png()?)?;
        eprintln!(
            "screenshot {}x{} -> {path}",
            shot.image.width(),
            shot.image.height()
        );
    }
    let trace = session.blocking_end_capture(capture)?;
    let validity = session.blocking_validity()?;
    session.blocking_shutdown()?;
    if let Some(path) = &args.trace {
        trace.write_jsonl(BufWriter::new(File::create(path)?))?;
    }
    let timeline = FrameTimeline::build(&trace);
    let summary = TraceSummary::build(&trace, &timeline);
    let mut out = std::io::stdout().lock();
    summary.write_json(&mut out)?;
    println!();
    println!("{}", serde_json::to_string_pretty(&validity)?);
    eprintln!("stdout:\n{}", app.stdout());
    eprintln!("stderr:\n{}", app.stderr());
    Ok(())
}
