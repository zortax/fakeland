//! The nested viewer inside a private headless sway: the window shows the output, its title
//! names the mode and the frame rate, and the sway cursor drives the session through it.
//!
//! The tests run only when `FAKELAND_TEST_HOST_DISPLAY` names the sway socket by absolute path
//! and `FAKELAND_TEST_SWAYSOCK` names its IPC socket. They never touch another display. Run
//! with `cargo test -p fakeland --features viewer --test viewer -- --ignored`.

#![cfg(feature = "viewer")]

mod support;
mod viewer_host;

use std::process::Command;
use std::time::Duration;

use fakeland::{
    Event, Fakeland, Image, LaunchSpec, OutputConfig, RendererChoice, Session, SessionConfig,
    WindowRef,
};
use fakeland_trace::InputSource;
use fakeland_types::{Hz, Nanos};
use serde_json::Value;

struct Host {
    display: String,
    swaysock: String,
}

fn host() -> Option<Host> {
    let display = std::env::var("FAKELAND_TEST_HOST_DISPLAY").ok()?;
    let swaysock = std::env::var("FAKELAND_TEST_SWAYSOCK").ok()?;
    if !display.starts_with('/') {
        eprintln!("FAKELAND_TEST_HOST_DISPLAY must be an absolute socket path");
        return None;
    }
    Some(Host { display, swaysock })
}

impl Host {
    fn swaymsg(&self, args: &[&str]) -> String {
        let out = Command::new("swaymsg")
            .env("SWAYSOCK", &self.swaysock)
            .env_remove("WAYLAND_DISPLAY")
            .args(args)
            .output()
            .expect("swaymsg");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    fn grim(&self, name: &str) -> Image {
        let path = support::out_dir().join(name);
        let status = Command::new("grim")
            .env("WAYLAND_DISPLAY", &self.display)
            .env_remove("DISPLAY")
            .arg(&path)
            .status()
            .expect("grim");
        assert!(status.success());
        eprintln!("grim: {}", path.display());
        Image::decode_png(&std::fs::read(&path).unwrap()).unwrap()
    }

    /// Returns the viewer windows of the sway tree: (name, x, y, w, h).
    fn viewer_windows(&self) -> Vec<(String, i64, i64, i64, i64)> {
        let tree: Value = serde_json::from_str(&self.swaymsg(&["-t", "get_tree", "-r"])).unwrap();
        let mut out = Vec::new();
        collect(&tree, &mut out);
        out
    }
}

fn collect(node: &Value, out: &mut Vec<(String, i64, i64, i64, i64)>) {
    if node["app_id"] == "fakeland-viewer" {
        let r = &node["rect"];
        out.push((
            node["name"].as_str().unwrap_or_default().to_string(),
            r["x"].as_i64().unwrap(),
            r["y"].as_i64().unwrap(),
            r["width"].as_i64().unwrap(),
            r["height"].as_i64().unwrap(),
        ));
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(list) = node[key].as_array() {
            for n in list {
                collect(n, out);
            }
        }
    }
}

fn session(host: &Host, renderer: RendererChoice) -> Session {
    let config = SessionConfig {
        viewer: true,
        viewer_display: Some(host.display.clone()),
        renderer,
        ..Fakeland::builder()
            .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
            .build()
    };
    Fakeland::start(config).expect("start")
}

/// Maps an output point to the sway layout through the letterboxed content of a window.
fn to_host(win: &(String, i64, i64, i64, i64), out: (f64, f64), x: f64, y: f64) -> (i64, i64) {
    let (_, wx, wy, ww, wh) = *win;
    let scale = (ww as f64 / out.0).min(wh as f64 / out.1);
    let (cw, ch) = ((out.0 * scale).round(), (out.1 * scale).round());
    let (cx, cy) = (
        ((ww as f64 - cw) / 2.0).floor(),
        ((wh as f64 - ch) / 2.0).floor(),
    );
    (
        (wx as f64 + cx + x * scale).round() as i64,
        (wy as f64 + cy + y * scale).round() as i64,
    )
}

fn color_near(px: [u8; 4], rgb: [u8; 3]) -> bool {
    (0..3).all(|c| px[c].abs_diff(rgb[c]) <= 12)
}

fn hover_through_the_viewer(renderer: RendererChoice, tag: &str) {
    let Some(host) = host() else {
        eprintln!("no test host display");
        return;
    };
    let _guard = support::serial();
    let session = session(&host, renderer);
    assert!(session.blocking_trace_snapshot().unwrap().header.viewer);
    let _client = session
        .blocking_launch(LaunchSpec::new(support::testclient()).args(["--scene", "hover"]))
        .expect("client");
    session
        .blocking_wait_window(WindowRef::Any, Nanos::from_secs(10))
        .expect("window");
    std::thread::sleep(Duration::from_millis(800));
    assert!(session.blocking_viewer_running().unwrap());
    let wins = host.viewer_windows();
    eprintln!("viewer windows: {wins:?}");
    assert_eq!(wins.len(), 1);
    let win = wins[0].clone();
    assert!(
        win.0.starts_with("fakeland DP-1 1280x720@60.00"),
        "{}",
        win.0
    );

    // The hover rect of the reference client sits at logical (100, 100, 200, 150).
    let inside = to_host(&win, (1280.0, 720.0), 200.0, 175.0);
    let shot = host.grim(&format!("viewer-{tag}-idle.png"));
    let px = shot.pixel(inside.0 as u32, inside.1 as u32).unwrap();
    assert!(
        color_near(px, [0x30, 0x60, 0xc0]),
        "idle hover color {px:?}"
    );

    let capture = session.blocking_begin_capture().unwrap();
    let mut pointer = viewer_host::HostPointer::new(&host.display, (1600, 1000));
    pointer.move_to(inside.0, inside.1);
    std::thread::sleep(Duration::from_millis(500));
    let shot = host.grim(&format!("viewer-{tag}-hover.png"));
    let px = shot.pixel(inside.0 as u32, inside.1 as u32).unwrap();
    assert!(
        color_near(px, [0xe0, 0x60, 0x20]),
        "active hover color {px:?}"
    );
    pointer.click();
    std::thread::sleep(Duration::from_millis(200));
    let trace = session.blocking_end_capture(capture).unwrap();
    let viewer_inputs: Vec<&Event> = trace
        .records
        .iter()
        .map(|r| &r.event)
        .filter(|e| {
            matches!(
                e,
                Event::InputSent {
                    source: InputSource::Viewer,
                    ..
                }
            )
        })
        .collect();
    eprintln!("viewer inputs: {}", viewer_inputs.len());
    let motion = viewer_inputs.iter().rev().find_map(|e| match e {
        Event::InputSent {
            kind: fakeland_trace::InputKind::PointerMotion,
            x,
            y,
            ..
        } => Some((x.unwrap_or(0.0), y.unwrap_or(0.0))),
        _ => None,
    });
    let (mx, my) = motion.expect("a viewer motion");
    // The last motion is the move; an earlier one can be the enter at the old cursor place.
    eprintln!("viewer motion at surface ({mx:.1}, {my:.1})");
    assert!((mx - 200.0).abs() < 2.0 && (my - 175.0).abs() < 2.0);
    assert!(viewer_inputs.iter().any(|e| matches!(
        e,
        Event::InputSent {
            kind: fakeland_trace::InputKind::PointerButton,
            ..
        }
    )));
    let overhead = support::count(
        &trace,
        |e| matches!(e, Event::CompositorOverhead { what, .. } if what == "viewer composite"),
    );
    eprintln!("viewer composites in the capture: {overhead}");
    assert!(overhead >= 1);
    session.blocking_shutdown().unwrap();
    std::thread::sleep(Duration::from_millis(300));
    assert!(
        host.viewer_windows().is_empty(),
        "the window closes with the session"
    );
}

#[test]
#[ignore = "needs a private headless sway"]
fn gles_viewer_shows_and_forwards_input() {
    hover_through_the_viewer(RendererChoice::Gles, "gles");
}

#[test]
#[ignore = "needs a private headless sway"]
fn pixman_viewer_uses_shared_memory() {
    hover_through_the_viewer(RendererChoice::Pixman, "pixman");
}

#[test]
#[ignore = "needs a private headless sway and vkcube"]
fn viewer_shows_vkcube_with_its_frame_rate() {
    let Some(host) = host() else {
        eprintln!("no test host display");
        return;
    };
    if !support::have("vkcube") {
        return;
    }
    let _guard = support::serial();
    let session = session(&host, RendererChoice::Gles);
    let _cube = session
        .blocking_launch(LaunchSpec::new("vkcube").args(["--wsi", "wayland"]))
        .expect("vkcube");
    session
        .blocking_wait_window(WindowRef::Any, Nanos::from_secs(10))
        .expect("window");
    fakeland::block_on(session.reset_validity()).unwrap();
    std::thread::sleep(Duration::from_millis(2000));
    let wins = host.viewer_windows();
    eprintln!("viewer windows: {wins:?}");
    let title = &wins[0].0;
    let fps: f64 = title
        .rsplit_once(' ')
        .map(|(rest, _)| rest)
        .and_then(|rest| rest.rsplit_once(' '))
        .and_then(|(_, n)| n.parse().ok())
        .expect("fps in the title");
    assert!(title.contains("vkcube"), "{title}");
    assert!((55.0..=65.0).contains(&fps), "{title}");
    let a = host.grim("viewer-vkcube-a.png");
    std::thread::sleep(Duration::from_millis(300));
    let b = host.grim("viewer-vkcube-b.png");
    assert!(support::distinct_colors(&a) > 20, "the cube shows");
    let changed = (0..a.height())
        .step_by(8)
        .flat_map(|y| (0..a.width()).step_by(8).map(move |x| (x, y)))
        .filter(|&(x, y)| a.pixel(x, y) != b.pixel(x, y))
        .count();
    assert!(changed > 50, "the cube moves in the viewer: {changed}");
    let validity = session.blocking_validity().unwrap();
    eprintln!(
        "lateness with the viewer: max {:.3} ms, late wakes {}",
        validity.max_lateness.as_millis_f64(),
        validity.late_wakes
    );
    session.blocking_shutdown().unwrap();
}

#[test]
#[ignore = "needs a private headless sway"]
fn closing_the_window_ends_only_the_viewer() {
    let Some(host) = host() else {
        eprintln!("no test host display");
        return;
    };
    let _guard = support::serial();
    let session = session(&host, RendererChoice::Gles);
    let _client = session
        .blocking_launch(LaunchSpec::new(support::testclient()).args(["--scene", "hover"]))
        .expect("client");
    session
        .blocking_wait_window(WindowRef::Any, Nanos::from_secs(10))
        .expect("window");
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(host.viewer_windows().len(), 1);
    host.swaymsg(&["[app_id=\"fakeland-viewer\"]", "kill"]);
    std::thread::sleep(Duration::from_millis(500));
    assert!(!session.blocking_viewer_running().unwrap());
    assert!(host.viewer_windows().is_empty());
    let shot = session
        .blocking_screenshot(Default::default(), Default::default())
        .expect("the session goes on headless");
    assert_eq!((shot.image.width(), shot.image.height()), (1280, 720));
    session.blocking_shutdown().unwrap();
}

#[test]
#[ignore = "needs a private headless sway"]
fn hotplugged_outputs_get_their_own_window() {
    let Some(host) = host() else {
        eprintln!("no test host display");
        return;
    };
    let _guard = support::serial();
    let session = session(&host, RendererChoice::Gles);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(host.viewer_windows().len(), 1);
    let second = session
        .blocking_add_output(OutputConfig::new("HDMI-A-1").mode(800, 600, Hz(75.0)))
        .expect("add output");
    std::thread::sleep(Duration::from_millis(500));
    let names: Vec<String> = host.viewer_windows().into_iter().map(|w| w.0).collect();
    eprintln!("viewer windows: {names:?}");
    assert_eq!(names.len(), 2);
    assert!(
        names
            .iter()
            .any(|n| n.starts_with("fakeland HDMI-A-1 800x600@75.00"))
    );
    session
        .blocking_remove_output(second)
        .expect("remove output");
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(host.viewer_windows().len(), 1);
    session.blocking_shutdown().unwrap();
}

#[test]
#[ignore = "needs a private headless sway and wtype"]
fn host_keys_reach_the_focused_window() {
    let Some(host) = host() else {
        eprintln!("no test host display");
        return;
    };
    if !support::have("wtype") {
        return;
    }
    let _guard = support::serial();
    let session = session(&host, RendererChoice::Gles);
    let _client = session
        .blocking_launch(LaunchSpec::new(support::testclient()).args(["--scene", "type"]))
        .expect("client");
    session
        .blocking_wait_window(WindowRef::Any, Nanos::from_secs(10))
        .expect("window");
    std::thread::sleep(Duration::from_millis(300));
    let capture = session.blocking_begin_capture().unwrap();
    let status = Command::new("wtype")
        .env("WAYLAND_DISPLAY", &host.display)
        .env_remove("DISPLAY")
        .args(["-s", "200", "hi"])
        .status()
        .expect("wtype");
    assert!(status.success());
    std::thread::sleep(Duration::from_millis(300));
    let trace = session.blocking_end_capture(capture).unwrap();
    let keys: Vec<&str> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::InputSent {
                kind: fakeland_trace::InputKind::Key,
                source: InputSource::Viewer,
                detail,
                ..
            } => Some(detail.as_str()),
            _ => None,
        })
        .collect();
    eprintln!("viewer keys: {keys:?}");
    assert_eq!(keys.len(), 4, "two presses and two releases");
    // The host keymap of wtype puts h and i on its own keycodes; the viewer presses the keys
    // with those keysyms in the session keymap: KEY_H is 35 and KEY_I is 23.
    assert!(keys[0].starts_with("keycode 35 pressed"), "{keys:?}");
    assert!(keys[2].starts_with("keycode 23 pressed"), "{keys:?}");
    session.blocking_shutdown().unwrap();
}
