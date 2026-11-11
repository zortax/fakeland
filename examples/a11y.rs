//! Runs a command in a headless session, waits for its window and prints its accessibility tree.
//!
//! Usage: `cargo run -p fakeland-examples --example a11y -- [--scale S] [--depth N] [--all]
//! [--json] [--settle MS] [--shot out.png] [--click NAME] [--hold SECS] [--screen-reader]
//! [--focused] -- <command> [args...]`
//!
//! `--all` keeps nodes without the `showing` state. `--click NAME` clicks the first node whose
//! name matches, through its action, and prints the tree again. `--hold SECS` keeps the session
//! open at the end and prints the bus addresses for other tools. `--screen-reader` makes the
//! session report a running screen reader. `--focused` reads the focused window in place of the
//! first window of the app.

use std::time::Duration;

use fakeland::accessibility::{Query, TextOptions, TreeOptions};
use fakeland::{
    A11yTarget, CaptureOptions, CaptureTarget, ClickVia, Fakeland, LaunchSpec, OutputConfig,
    WindowRef,
};
use fakeland_types::{Hz, Nanos};

struct Args {
    scale: f64,
    depth: usize,
    all: bool,
    json: bool,
    settle: u64,
    shot: Option<String>,
    click: Option<String>,
    hold: u64,
    screen_reader: bool,
    focused: bool,
    command: Vec<String>,
}

fn parse() -> Result<Args, String> {
    let mut args = Args {
        scale: 1.0,
        depth: 32,
        all: false,
        json: false,
        settle: 1500,
        shot: None,
        click: None,
        hold: 0,
        screen_reader: false,
        focused: false,
        command: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{a} needs a value"));
        match a.as_str() {
            "--scale" => args.scale = value()?.parse().map_err(|e| format!("--scale: {e}"))?,
            "--depth" => args.depth = value()?.parse().map_err(|e| format!("--depth: {e}"))?,
            "--settle" => args.settle = value()?.parse().map_err(|e| format!("--settle: {e}"))?,
            "--all" => args.all = true,
            "--screen-reader" => args.screen_reader = true,
            "--focused" => args.focused = true,
            "--json" => args.json = true,
            "--shot" => args.shot = Some(value()?),
            "--click" => args.click = Some(value()?),
            "--hold" => args.hold = value()?.parse().map_err(|e| format!("--hold: {e}"))?,
            "--" => {
                args.command = it.collect();
                break;
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if args.command.is_empty() {
        return Err("give a command after --".into());
    }
    Ok(args)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse()?;
    let session = Fakeland::builder()
        .output(
            OutputConfig::new("DP-1")
                .mode(1280, 800, Hz(60.0))
                .scale(args.scale),
        )
        .screen_reader(args.screen_reader)
        .start()?;
    let spec = LaunchSpec::from_argv(args.command.clone()).ok_or("empty command")?;
    let app = session.blocking_launch(spec)?;
    let window = app.blocking_wait_window(Nanos::from_secs(20))?;
    eprintln!(
        "window {} {:?} at {:?}",
        window.id, window.title, window.geometry
    );
    std::thread::sleep(Duration::from_millis(args.settle));
    let a11y = session.blocking_a11y()?;
    for app in a11y.blocking_apps()? {
        eprintln!(
            "app {:?} pid {:?} toolkit {:?} frames {:?}",
            app.name,
            app.pid,
            app.toolkit,
            app.frames
                .iter()
                .map(|f| (&f.role, &f.name, f.window))
                .collect::<Vec<_>>()
        );
    }
    let target = A11yTarget::Window {
        window: if args.focused {
            WindowRef::Focused
        } else {
            WindowRef::Id { id: window.id }
        },
    };
    let opts = TreeOptions::default().depth(args.depth).showing(!args.all);
    let print = |label: &str| -> Result<(), Box<dyn std::error::Error>> {
        let tree = a11y.blocking_tree(&target, &opts)?;
        eprintln!("== {label}: {} nodes", tree.count());
        if args.json {
            println!("{}", serde_json::to_string_pretty(&tree)?);
        } else {
            print!("{}", a11y.client().render(&tree, &TextOptions::default()));
        }
        Ok(())
    };
    print("tree")?;
    if let Some(shot) = &args.shot {
        let s = session.blocking_screenshot(
            CaptureTarget::Output { output: None },
            CaptureOptions::default(),
        )?;
        std::fs::write(shot, s.image.encode_png()?)?;
    }
    if let Some(name) = &args.click {
        let query = Query::default().with_name(name.as_str());
        let found = a11y.blocking_find(&target, &query, &opts, 1)?;
        let node = found.first().ok_or("no node matches --click")?;
        let outcome = a11y.blocking_click(&node.id, ClickVia::Auto)?;
        eprintln!("clicked {} via {:?}", node.id, outcome.via);
        std::thread::sleep(Duration::from_millis(500));
        if app.try_wait().is_some() {
            eprintln!("the app exited after the click");
        } else {
            print("after click")?;
        }
    }
    if args.hold > 0 {
        eprintln!(
            "WAYLAND_DISPLAY={} DBUS_SESSION_BUS_ADDRESS={} AT_SPI_BUS={}",
            session.socket_name(),
            session.dbus_address().unwrap_or(""),
            a11y.client().address()
        );
        std::thread::sleep(Duration::from_secs(args.hold));
    }
    session.blocking_shutdown()?;
    Ok(())
}
