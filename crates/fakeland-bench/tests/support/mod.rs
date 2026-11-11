//! Shared helpers of the integration tests: one session at a time, the reference client and a
//! session with it running.

#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use fakeland::{App, Fakeland, LaunchSpec, OutputConfig, Session, WindowInfo, WindowRef};
use fakeland_types::{Hz, Nanos};

static SESSION_LOCK: Mutex<()> = Mutex::new(());

/// Runs one session at a time, so timing checks see an idle machine.
pub fn serial() -> MutexGuard<'static, ()> {
    SESSION_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Returns the directory for reports and traces of the tests.
pub fn out_dir() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("fakeland-bench-it");
    std::fs::create_dir_all(&dir).expect("output dir");
    dir
}

/// Returns the path of the reference client.
pub fn testclient() -> String {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/debug/fakeland-testclient");
    assert!(
        path.exists(),
        "build the reference client first: cargo build -p fakeland-testclient"
    );
    path.to_string_lossy().into_owned()
}

/// Starts a 1280x720 60 Hz session with app marks.
pub fn session() -> Session {
    Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .app_marks(true)
        .start()
        .expect("start")
}

/// Launches the reference client with an app id and arguments and waits for its window.
pub fn client(session: &Session, app_id: &str, args: &[&str]) -> (App, WindowInfo) {
    let mut argv = vec!["--app-id", app_id, "--exit-after", "120"];
    argv.extend_from_slice(args);
    let app = session
        .blocking_launch(LaunchSpec::new(testclient()).args(argv))
        .expect("launch");
    let window = session
        .blocking_wait_window(WindowRef::app_id(app_id), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    (app, window)
}

/// Writes a report as JSON and text into the output directory.
pub fn save(report: &fakeland_bench::BenchReport, name: &str) {
    let dir = out_dir();
    std::fs::write(dir.join(format!("{name}.json")), report.to_json_pretty()).expect("json");
    let text = report.to_text();
    std::fs::write(dir.join(format!("{name}.txt")), &text).expect("text");
    eprintln!("{text}");
}
