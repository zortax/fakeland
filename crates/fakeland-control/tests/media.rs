//! End-to-end test of the resource and recording methods over the control socket.

use std::path::PathBuf;

use fakeland::{
    Fakeland, LaunchSpec, RecordSpec, RendererChoice, SamplingConfig, SessionConfig, WindowRef,
};
use fakeland_control::methods::*;
use fakeland_control::wire::ErrorCode;
use fakeland_control::{ClientError, ControlClient, serve_session};
use fakeland_types::{Dur, Hz, Nanos};

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
fn records_and_samples_over_the_socket() {
    let config = SessionConfig {
        renderer: RendererChoice::Pixman,
        sampling: Some(SamplingConfig::every(Nanos::from_millis(100))),
        ..SessionConfig::default()
    };
    let session = Fakeland::start(config).expect("session");
    let dir = tempfile::Builder::new()
        .prefix("flmedia")
        .tempdir_in("/tmp")
        .unwrap();
    let path = dir.path().join("c.sock");
    let server = serve_session(session, Some(&path)).expect("serve");
    let c = ControlClient::connect(&path).expect("connect");
    c.blocking_call::<Launch>(LaunchParams {
        spec: LaunchSpec::new(testclient()).args(["--scene", "animate"]),
        wait_window: Some(Dur::from_secs(10)),
    })
    .unwrap();

    let relative = c.blocking_call::<RecordStart>(RecordSpec::output("rel.y4m"));
    assert!(matches!(
        relative,
        Err(ClientError::Remote(ref b)) if b.code == ErrorCode::InvalidParams
    ));
    let missing = c.blocking_call::<RecordStop>(RecordStopParams::default());
    assert!(matches!(
        missing,
        Err(ClientError::Remote(ref b)) if b.code == ErrorCode::NotFound
    ));

    let video = dir.path().join("w.y4m");
    let info = c
        .blocking_call::<RecordStart>(
            RecordSpec::window(WindowRef::Any, &video)
                .fps(Hz(30.0))
                .max_dim(480),
        )
        .unwrap();
    assert_eq!((info.width, info.height), (480, 270));
    let list = c.blocking_call::<Recordings>(NoParams {}).unwrap();
    assert_eq!(list.len(), 1);
    std::thread::sleep(std::time::Duration::from_millis(1000));
    let snap = c.blocking_call::<Resources>(NoParams {}).unwrap();
    assert!(snap.enabled);
    assert_eq!(snap.processes.len(), 1);
    assert!(snap.processes[0].rss_bytes > 0);
    let report = c
        .blocking_call::<RecordStop>(RecordStopParams { id: Some(info.id) })
        .unwrap();
    assert!(report.ok(), "{:?}", report.error);
    assert!((29..=32).contains(&report.frames), "{}", report.frames);
    assert!(report.rendered >= report.frames - 1);
    let bytes = std::fs::metadata(&video).unwrap().len();
    let header = "YUV4MPEG2 W480 H270 F30:1 Ip A1:1 C420jpeg\n".len() as u64;
    assert_eq!(bytes, header + report.frames * (6 + 480 * 270 * 3 / 2));
    assert!(
        c.blocking_call::<Recordings>(NoParams {})
            .unwrap()
            .is_empty()
    );

    c.blocking_call::<Shutdown>(NoParams {}).unwrap();
    server.wait();
}
