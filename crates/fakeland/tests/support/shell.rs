//! Helpers of the shell tests: sessions, the layer client and the reference toplevel, the JSON
//! event logs of clients, polls, pixel reads and clicks.

use std::time::Duration;

use fakeland::{
    App, CaptureOptions, CaptureTarget, Fakeland, LaunchSpec, OutputConfig, Session, WindowInfo,
    WindowRef,
};
use fakeland_scenario::{Button, Position};
use fakeland_types::{Hz, Nanos};
use serde_json::Value;

/// Starts a session with one output of a size.
pub fn session(width: i32, height: i32) -> Session {
    Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(width, height, Hz(60.0)))
        .start()
        .expect("start")
}

/// Returns the launch of the layer client with arguments.
pub fn layer(args: &[&str]) -> LaunchSpec {
    LaunchSpec::new(super::layerclient()).args(args.iter().copied())
}

/// Launches the reference client as a toplevel and waits for its window.
pub fn toplevel(session: &Session, app_id: &str) -> (App, WindowInfo) {
    let app = session
        .blocking_launch(LaunchSpec::new(super::testclient()).args([
            "--scene",
            "click",
            "--app-id",
            app_id,
            "--exit-after",
            "30",
        ]))
        .expect("launch");
    let window = session
        .blocking_wait_window(WindowRef::app_id(app_id), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    (app, window)
}

/// Waits for the layer surface of a namespace.
pub fn wait_layer(session: &Session, app: &App, namespace: &str) -> WindowInfo {
    session
        .blocking_wait_window(WindowRef::layer(namespace), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no layer surface ({e}): {}", app.stderr()))
}

/// Returns the JSON events of a client log.
pub fn events(app: &App) -> Vec<Value> {
    app.stdout()
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Polls a condition every 20 ms for up to 5 s and returns its first value.
pub fn poll<T>(what: impl Fn() -> String, mut f: impl FnMut() -> Option<T>) -> T {
    for _ in 0..250 {
        if let Some(v) = f() {
            return v;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("{}", what())
}

/// Waits until a client logged an event that matches.
pub fn wait_event(app: &App, what: &str, f: impl Fn(&Value) -> bool) -> Value {
    poll(
        || format!("no {what} event: {}\n{}", app.stdout(), app.stderr()),
        || events(app).into_iter().find(|e| f(e)),
    )
}

/// Returns true when a log event has a name.
pub fn is(e: &Value, event: &str) -> bool {
    e["event"] == event
}

/// Returns the count of log events that match.
pub fn count_events(app: &App, f: impl Fn(&Value) -> bool) -> usize {
    events(app).iter().filter(|e| f(e)).count()
}

/// Waits until a window matches a condition.
pub fn wait_info(session: &Session, id: WindowRef, f: impl Fn(&WindowInfo) -> bool) -> WindowInfo {
    poll(
        || format!("{id:?} never matched"),
        || {
            session
                .blocking_wait_window(id.clone(), Nanos::from_secs(5))
                .ok()
                .filter(|w| f(w))
        },
    )
}

/// Returns the color of a pixel of a screenshot without the cursor.
pub fn pixel_at(session: &Session, x: u32, y: u32) -> [u8; 3] {
    let shot = session
        .blocking_screenshot(
            CaptureTarget::default(),
            CaptureOptions {
                include_cursor: false,
                ..CaptureOptions::default()
            },
        )
        .expect("screenshot");
    let px = shot.image.pixel(x, y).expect("pixel");
    [px[0], px[1], px[2]]
}

/// Waits until a pixel shows a color.
pub fn wait_pixel(session: &Session, x: u32, y: u32, color: [u8; 3]) {
    poll(
        || {
            format!(
                "pixel {x},{y} is {:?}, expected {color:?}",
                pixel_at(session, x, y)
            )
        },
        || (pixel_at(session, x, y) == color).then_some(()),
    );
}

/// Clicks the left button at an output position.
pub fn click(session: &Session, x: f64, y: f64) {
    session
        .blocking_click(Position::output(x, y), Button::Left)
        .expect("click");
}
