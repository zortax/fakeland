//! Display timing modes with the reference client: unthrottled outputs present far faster than
//! 60 Hz, manual outputs present only on `step` and take screenshots without output wakes. Run
//! with `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::{
    CaptureOptions, CaptureTarget, Event, Fakeland, LaunchSpec, OutputConfig, TimingConfig,
    WindowRef,
};
use fakeland_trace::FrameTimeline;
use fakeland_types::{Hz, Nanos};

fn client() -> LaunchSpec {
    LaunchSpec::new(support::testclient()).args(["--scene", "animate", "--exit-after", "30"])
}

#[test]
#[ignore = "runs the reference client"]
fn unthrottled_output_presents_far_above_60_hz() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(
            OutputConfig::new("DP-1")
                .mode(640, 480, Hz(60.0))
                .timing(TimingConfig::unthrottled()),
        )
        .start()
        .expect("start");
    let app = session.blocking_launch(client()).expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let capture = session.blocking_begin_capture().expect("capture");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let trace = session.blocking_end_capture(capture).expect("trace");
    session.blocking_shutdown().expect("shutdown");
    let timeline = FrameTimeline::build(&trace);
    let frames = timeline.presented_frames(window.surface, None);
    assert!(frames.len() > 10, "{} frames", frames.len());
    let span = frames
        .last()
        .unwrap()
        .time
        .since(frames[0].time)
        .as_secs_f64();
    let fps = (frames.len() - 1) as f64 / span;
    eprintln!("unthrottled {fps:.0} fps");
    assert!(fps > 240.0, "{fps:.0} fps");
    let no_vsync = support::records(&trace, |e| matches!(e, Event::Presented { .. })).all(
        |r| matches!(r.event, Event::Presented { flags, refresh: None, .. } if flags & 1 == 0),
    );
    assert!(
        no_vsync,
        "unthrottled presentation has no vsync flag and no refresh"
    );
}

#[test]
#[ignore = "runs the reference client"]
fn manual_output_presents_only_on_step() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(
            OutputConfig::new("DP-1")
                .mode(640, 480, Hz(60.0))
                .timing(TimingConfig::manual(Hz(60.0))),
        )
        .start()
        .expect("start");
    let app = session.blocking_launch(client()).expect("launch");
    let window = loop {
        let windows = session.blocking_windows().expect("windows");
        if let Some(w) = windows
            .into_iter()
            .find(|w| w.app_id == "fakeland-testclient")
        {
            break w;
        }
        assert!(app.try_wait().is_none(), "client ended: {}", app.stderr());
        std::thread::sleep(std::time::Duration::from_millis(20));
    };
    std::thread::sleep(std::time::Duration::from_millis(300));
    let presented = |trace: &fakeland::Trace| {
        support::count(
            trace,
            |e| matches!(e, Event::Presented { surface, .. } if *surface == window.surface),
        )
    };
    let capture = session.blocking_begin_capture().expect("capture");
    std::thread::sleep(std::time::Duration::from_millis(300));
    let idle = session.blocking_end_capture(capture).expect("trace");
    assert_eq!(presented(&idle), 0, "nothing presents without a step");

    let mut stepped = 0;
    for _ in 0..5 {
        let capture = session.blocking_begin_capture().expect("capture");
        session.blocking_step(None, 1).expect("step");
        std::thread::sleep(std::time::Duration::from_millis(50));
        let trace = session.blocking_end_capture(capture).expect("trace");
        stepped += presented(&trace);
        assert!(
            presented(&trace) <= 1,
            "one step presents at most one frame"
        );
    }
    assert!(stepped >= 4, "{stepped} frames over 5 steps");
    let info = session
        .blocking_wait_window(WindowRef::id(window.id), Nanos::from_secs(1))
        .expect("window");
    assert!(info.is_mapped());
    session.blocking_shutdown().expect("shutdown");
}

#[test]
#[ignore = "runs the reference client"]
fn fifo_client_presents_once_per_refresh() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(640, 480, Hz(60.0)))
        .start()
        .expect("start");
    let spec = LaunchSpec::new(support::testclient()).args([
        "--scene",
        "animate",
        "--mode",
        "unthrottled",
        "--fifo",
        "--exit-after",
        "30",
    ]);
    let app = session.blocking_launch(spec).expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let capture = session.blocking_begin_capture().expect("capture");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let trace = session.blocking_end_capture(capture).expect("trace");
    session.blocking_shutdown().expect("shutdown");
    let timeline = FrameTimeline::build(&trace);
    let frames = timeline.presented_frames(window.surface, None);
    let missed: u64 = frames.iter().filter_map(|f| f.vblanks_missed).sum();
    assert!(
        (55..=62).contains(&frames.len()),
        "{} frames in 1 s",
        frames.len()
    );
    assert!(missed <= 2, "{missed} missed vblanks");
    let fifo = support::count(&trace, |e| {
        matches!(
            e,
            Event::Commit {
                fifo_wait: true,
                ..
            }
        )
    });
    assert!(fifo > 50, "the client waits on fifo barriers");
}

#[test]
#[ignore = "runs the reference client"]
fn vrr_output_follows_the_client_rate() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(
            OutputConfig::new("DP-1")
                .mode(640, 480, Hz(144.0))
                .timing(TimingConfig::vrr(Hz(48.0), Hz(144.0))),
        )
        .start()
        .expect("start");
    let spec = LaunchSpec::new(support::testclient()).args([
        "--scene",
        "animate",
        "--mode",
        "fixed-rate",
        "--rate",
        "90",
        "--exit-after",
        "30",
    ]);
    let app = session.blocking_launch(spec).expect("launch");
    let window = app
        .blocking_wait_window(Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let capture = session.blocking_begin_capture().expect("capture");
    std::thread::sleep(std::time::Duration::from_secs(1));
    let trace = session.blocking_end_capture(capture).expect("trace");
    session.blocking_shutdown().expect("shutdown");
    let timeline = FrameTimeline::build(&trace);
    let frames = timeline.presented_frames(window.surface, None);
    let span = frames
        .last()
        .unwrap()
        .time
        .since(frames[0].time)
        .as_secs_f64();
    let fps = (frames.len() - 1) as f64 / span;
    eprintln!("vrr {fps:.1} fps");
    assert!(
        (80.0..=100.0).contains(&fps),
        "{fps:.1} fps follows the 90 Hz client"
    );
}

#[test]
#[ignore = "runs the reference client"]
fn manual_output_takes_screenshots() {
    let _serial = support::serial();
    let session = Fakeland::builder()
        .output(
            OutputConfig::new("DP-1")
                .mode(640, 480, Hz(60.0))
                .timing(TimingConfig::manual(Hz(60.0))),
        )
        .start()
        .expect("start");
    let _app = session.blocking_launch(client()).expect("launch");
    session.blocking_step(None, 2).expect("step");
    std::thread::sleep(std::time::Duration::from_millis(300));
    // The session is idle now. A manual output has no wakes of its own, so the capture steps
    // must run without them.
    let (tx, rx) = std::sync::mpsc::channel();
    let s = session.clone();
    std::thread::spawn(move || {
        let _ = tx.send(s.blocking_screenshot(CaptureTarget::default(), CaptureOptions::default()));
    });
    let shot = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the screenshot returns")
        .expect("screenshot");
    assert_eq!((shot.image.width(), shot.image.height()), (640, 480));
    session.blocking_shutdown().expect("shutdown");
}
