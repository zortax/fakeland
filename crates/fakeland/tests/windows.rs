//! Window management with the reference client: kiosk configures, window operations, the
//! suspended state, launch outputs, close and settle waits after input. Run with
//! `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::{
    CaptureOptions, CaptureTarget, Event, Fakeland, LaunchSpec, OutputConfig, Placement, WindowOp,
    WindowRef,
};
use fakeland_scenario::Position;
use fakeland_types::{Hz, Logical, Nanos, Size, SurfaceId};

fn client(app_id: &str) -> LaunchSpec {
    LaunchSpec::new(support::testclient()).args([
        "--scene",
        "resize",
        "--app-id",
        app_id,
        "--exit-after",
        "30",
    ])
}

fn configures(trace: &fakeland::Trace, surface: SurfaceId) -> Vec<(i32, i32, Vec<String>)> {
    support::records(
        trace,
        |e| matches!(e, Event::Configure { surface: s, .. } if *s == surface),
    )
    .map(|r| match &r.event {
        Event::Configure {
            width,
            height,
            states,
            ..
        } => (*width, *height, states.clone()),
        _ => unreachable!(),
    })
    .collect()
}

#[test]
#[ignore = "runs the reference client"]
fn kiosk_window_operations() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(
            OutputConfig::new("DP-1")
                .mode(1600, 900, Hz(60.0))
                .scale(1.25),
        )
        .start()
        .expect("start");
    let app = session
        .blocking_launch(client("org.example.one"))
        .expect("launch");
    let window = session
        .blocking_wait_window(WindowRef::app_id("org.example.one"), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    assert_eq!(window.placement, Placement::Maximized);
    assert!(window.focused);
    assert_eq!((window.geometry.w, window.geometry.h), (1280, 720));

    let me = WindowRef::id(window.id);
    session
        .blocking_window_op(
            me.clone(),
            WindowOp::Resize {
                size: Size::<Logical>::new(640, 400),
            },
        )
        .expect("resize");
    session
        .blocking_window_op(me.clone(), WindowOp::Suspend { suspended: true })
        .expect("suspend");
    session
        .blocking_window_op(me.clone(), WindowOp::Fullscreen)
        .expect("fullscreen");
    session
        .blocking_wait_frames(me.clone(), 2, Nanos::from_secs(5))
        .expect("frames");
    let trace = session.blocking_trace_snapshot().expect("trace");
    let list = configures(&trace, window.surface);
    assert_eq!(list[0].0, 1280, "the first configure fills the output");
    assert!(list[0].2.iter().any(|s| s == "maximized"));
    assert!(list[0].2.iter().any(|s| s == "activated"));
    assert!(
        list.iter()
            .any(|(w, h, s)| (*w, *h) == (640, 400) && !s.iter().any(|x| x == "maximized"))
    );
    assert!(
        list.iter()
            .any(|(_, _, s)| s.iter().any(|x| x == "suspended"))
    );
    assert!(list.last().unwrap().2.iter().any(|s| s == "fullscreen"));
    // The client acks only the newest of several pending configures, so the three operations
    // in a row give at least the initial ack and one more.
    assert!(
        support::count(
            &trace,
            |e| matches!(e, Event::AckConfigure { surface, .. } if *surface == window.surface)
        ) >= 2
    );

    session
        .blocking_window_op(me, WindowOp::Close)
        .expect("close");
    let exit = app.blocking_wait();
    assert!(exit.code.is_some() || exit.signal.is_some());
    session.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the reference client"]
fn launch_output_and_window_moves() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .output(
            OutputConfig::new("DP-2")
                .mode(1920, 1080, Hz(144.0))
                .scale(1.5),
        )
        .start()
        .expect("start");
    let outputs = session.blocking_outputs().expect("outputs");
    let second = outputs[1].id;
    assert_eq!(outputs[1].position.x, 1280);
    let app = session
        .blocking_launch(client("org.example.two").output(second))
        .expect("launch");
    let window = session
        .blocking_wait_window(WindowRef::app_id("org.example.two"), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    assert_eq!(window.output, second);
    assert_eq!(window.geometry.x, 1280);
    assert_eq!((window.geometry.w, window.geometry.h), (1280, 720));
    let windows = app.blocking_windows().expect("app windows");
    assert_eq!(windows.len(), 1);

    let first = outputs[0].id;
    session
        .blocking_window_op(
            WindowRef::id(window.id),
            WindowOp::MoveToOutput { output: first },
        )
        .expect("move");
    session
        .blocking_wait_frames(WindowRef::id(window.id), 3, Nanos::from_secs(5))
        .expect("frames on the new output");
    let moved = session
        .blocking_wait_window(WindowRef::id(window.id), Nanos::from_secs(1))
        .expect("window");
    assert_eq!(moved.output, first);
    assert_eq!(moved.geometry.x, 0);
    let trace = session.blocking_trace_snapshot().expect("trace");
    let on_first = support::count(
        &trace,
        |e| matches!(e, Event::Presented { surface, output, .. } if *surface == window.surface && *output == first),
    );
    assert!(on_first >= 1, "the window presents on its new output");
    session.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the reference client"]
fn floating_policy_places_windows_at_the_default_size() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1920, 1080, Hz(60.0)))
        .floating(Size::new(800, 600))
        .start()
        .expect("start");
    let app = session
        .blocking_launch(client("org.example.float"))
        .expect("launch");
    let window = session
        .blocking_wait_window(WindowRef::app_id("org.example.float"), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    assert_eq!(window.placement, Placement::Floating);
    assert_eq!((window.geometry.x, window.geometry.y), (560, 240));
    assert_eq!((window.geometry.w, window.geometry.h), (800, 600));
    session.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the reference client"]
fn commit_waits_see_commits_and_applies() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(800, 600, Hz(60.0)))
        .start()
        .expect("start");
    let app = session
        .blocking_launch(LaunchSpec::new(support::testclient()).args([
            "--scene",
            "animate",
            "--exit-after",
            "30",
        ]))
        .expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let me = WindowRef::id(window.id);
    let committed = session
        .blocking_wait_commit(me.clone(), false, Nanos::from_secs(2))
        .expect("commit");
    let applied = session
        .blocking_wait_commit(me.clone(), true, Nanos::from_secs(2))
        .expect("apply");
    assert!(applied >= committed);
    let trace = session.blocking_trace_snapshot().expect("trace");
    let latched = support::count(
        &trace,
        |e| matches!(e, Event::Latched { commit, .. } if *commit == applied),
    );
    assert_eq!(latched, 1, "a content commit applies at its latch");
    session.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the reference client"]
fn settle_after_input_sees_the_answer() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .start()
        .expect("start");
    let app = session
        .blocking_launch(LaunchSpec::new(support::testclient()).args([
            "--scene",
            "hover",
            "--mode",
            "on-demand",
            "--exit-after",
            "30",
        ]))
        .expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let quiet = Nanos::from_millis(100);
    let timeout = Nanos::from_secs(2);
    session
        .blocking_wait_settled(WindowRef::Any, quiet, timeout)
        .expect("settled");
    // The window has been quiet for longer than `quiet`. The settle after the move still waits
    // for the hover frame.
    session
        .blocking_pointer_move(Position::window(WindowRef::id(window.id), 200.0, 174.0))
        .expect("move");
    session
        .blocking_wait_settled(WindowRef::Any, quiet, timeout)
        .expect("settled after the move");
    let shot = session
        .blocking_screenshot(CaptureTarget::default(), CaptureOptions::default())
        .expect("screenshot");
    session.blocking_shutdown().expect("shutdown");
    let (x, y) = (
        window.geometry.x as u32 + 200,
        window.geometry.y as u32 + 174,
    );
    assert_eq!(
        shot.image.pixel(x, y).map(|p| [p[0], p[1], p[2]]),
        Some([0xe0, 0x60, 0x20]),
        "the screenshot shows the hover state"
    );
}
