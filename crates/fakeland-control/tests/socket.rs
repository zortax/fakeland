//! End-to-end test of the control socket: a session with the reference client, a server on a
//! temporary socket and a client that drives it.

use std::path::PathBuf;

use fakeland::{Fakeland, LaunchSpec, RendererChoice, SessionConfig};
use fakeland_control::methods::*;
use fakeland_control::{ControlClient, SubEvent, serve_session};
use fakeland_scenario::{Position, Scenario, WindowRef};
use fakeland_types::Dur;

fn testclient() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/fakeland-testclient");
    assert!(
        path.exists(),
        "build the reference client first: cargo build -p fakeland-testclient"
    );
    path.to_string_lossy().into_owned()
}

#[test]
#[ignore = "starts a session; run with `just it`"]
fn drives_a_session_over_the_socket() {
    let config = SessionConfig {
        renderer: RendererChoice::Pixman,
        ..SessionConfig::default()
    };
    let session = Fakeland::start(config).expect("session");
    let dir = tempfile::Builder::new()
        .prefix("flctl")
        .tempdir_in("/tmp")
        .unwrap();
    let path = dir.path().join("c.sock");
    let mut server = serve_session(session, Some(&path)).expect("serve");
    let c = ControlClient::connect(&path).expect("connect");

    let info = c.blocking_call::<Info>(NoParams {}).unwrap();
    assert_eq!(info.protocol, fakeland_control::wire::PROTOCOL_VERSION);

    let err = c.blocking_call_raw("no_such_method", serde_json::Value::Null);
    assert!(matches!(
        err,
        Err(fakeland_control::ClientError::Remote(ref b)) if b.code == fakeland_control::wire::ErrorCode::UnknownMethod
    ));

    let mut sub = c
        .blocking_subscribe(SubscribeParams {
            types: vec!["user_mark".into(), "window_mapped".into()],
            ..SubscribeParams::default()
        })
        .unwrap();

    let app = c
        .blocking_call::<Launch>(LaunchParams {
            spec: LaunchSpec::new(testclient()).args(["--scene", "hover", "--mode", "on-demand"]),
            wait_window: Some(Dur::from_secs(10)),
        })
        .unwrap();
    let window = app.window.expect("window");
    assert_eq!(window.app_id, "fakeland-testclient");

    c.blocking_call::<Mark>(MarkParams { label: "m1".into() })
        .unwrap();
    let mut saw = Vec::new();
    while saw.len() < 2 {
        match sub.blocking_next().expect("event") {
            SubEvent::Record(r) => saw.push(r.event.name()),
            SubEvent::Gap { .. } => {}
        }
    }
    assert_eq!(saw, ["window_mapped", "user_mark"]);

    let capture = c.blocking_call::<TraceBegin>(NoParams {}).unwrap();
    let sent = c
        .blocking_call::<PointerMove>(PointerMoveParams {
            pos: Position::window(WindowRef::Any, 150.0, 150.0),
        })
        .unwrap();
    assert_eq!(sent.len(), 1);
    c.blocking_call::<WaitFrames>(WaitFramesParams {
        window: WindowRef::Any,
        n: 1,
        timeout: Dur::from_secs(5),
    })
    .unwrap();
    let shot = c
        .blocking_call::<Screenshot>(ScreenshotParams {
            max_dim: Some(640),
            inline: true,
            ..ScreenshotParams::default()
        })
        .unwrap();
    assert_eq!((shot.width, shot.height), (640, 360));
    assert!((shot.scale - 3.0).abs() < 1e-9);
    assert!(shot.png_base64.is_some_and(|b| b.len() > 100));

    let trace_path = dir.path().join("t.jsonl");
    let trace = c
        .blocking_call::<TraceEnd>(TraceEndParams {
            capture: Some(capture.capture),
            output: Some(TraceOutput {
                path: trace_path.clone(),
                format: None,
            }),
        })
        .unwrap();
    assert!(trace.records > 0);
    assert!(trace_path.exists());
    assert!(trace.summary.inputs.count >= 1);

    let scenario = Scenario::from_json(
        r#"{"name": "s", "steps": [
            {"action": {"type": "pointer", "op": "move_to", "pos": {"type": "window", "x": 500, "y": 400}}},
            {"anchor": {"type": "next_vblank", "phase": "2ms"}, "action": {"type": "mark", "label": "after"}},
            {"action": {"type": "capture", "label": "end"}}
        ]}"#,
    )
    .unwrap();
    let mut runs = c.run_events();
    let summary = c
        .blocking_run_and_wait(RunScenarioParams {
            scenario,
            frames: Vec::new(),
            captures_dir: Some(dir.path().join("caps")),
            trace: None,
        })
        .unwrap();
    assert!(summary.ok, "{:?}", summary.error);
    assert_eq!(summary.steps.len(), 3);
    assert_eq!(summary.captures.len(), 1);
    assert!(summary.captures[0].shot.path.as_ref().unwrap().exists());
    let event = fakeland::block_on(futures_util::StreamExt::next(&mut runs)).unwrap();
    assert_eq!(event.run, summary.run);

    let stats = c
        .blocking_call::<FrameStatsOf>(FrameStatsParams {
            window: WindowRef::Any,
            last: Dur::from_secs(10),
        })
        .unwrap();
    assert!(stats.summary.surfaces.iter().any(|s| s.presented > 0));

    let logs = c.blocking_call::<AppLogs>(LogsParams::default()).unwrap();
    assert_eq!(logs.app.pid, app.pid);

    let killed = c
        .blocking_call::<Kill>(KillParams {
            pid: Some(app.pid),
            signal: KillSignal::Term,
            wait: Some(Dur::from_secs(3)),
        })
        .unwrap();
    assert!(killed.exit.is_some());

    c.blocking_call::<Shutdown>(NoParams {}).unwrap();
    assert!(server.wait_for(std::time::Duration::from_secs(5)));
    server.stop();
    assert!(!path.exists());
}
