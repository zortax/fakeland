//! End-to-end tests of fakeland-testclient against an in-process compositor: app marks match
//! the JSON log to the nanosecond, input ids reach the commit that shows their effect, input
//! delay, frame limits and exit codes.

mod support;

use std::path::PathBuf;

use fakeland_types::Nanos;
use serde_json::Value;
use support::{Commit, TestCompositor};

fn log_path(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "fakeland-testclient-{}-{name}.jsonl",
        std::process::id()
    ))
}

fn read_log(path: &PathBuf) -> Vec<Value> {
    let text = std::fs::read_to_string(path).expect("log");
    let _ = std::fs::remove_file(path);
    text.lines()
        .map(|l| serde_json::from_str(l).expect("json line"))
        .collect()
}

fn events<'a>(log: &'a [Value], name: &str) -> Vec<&'a Value> {
    log.iter().filter(|e| e["event"] == name).collect()
}

fn buffer_commits(commits: &[Commit]) -> Vec<&Commit> {
    commits.iter().filter(|c| c.buffer).collect()
}

const ID1: u64 = (1 << 40) + 5;
const ID2: u64 = (1 << 40) + 6;
const ID3: u64 = (1 << 40) + 7;

#[test]
fn marks_match_the_log() {
    let path = log_path("marks");
    let mut c = TestCompositor::start(true, &path, &["--scene", "hover", "--mode", "on-demand"]);
    c.pump_until("first frame", |s| s.commits.iter().any(|c| c.buffer));
    c.pump_until("pointer", |s| {
        !s.pointers.is_empty() && !s.managers.is_empty()
    });
    assert!(c.state.client_marks.contains(&"connected".to_owned()));
    assert!(c.state.client_marks.contains(&"configured".to_owned()));
    let first = buffer_commits(&c.state.commits)[0].clone();
    assert_eq!(first.marks.frame, Some(1));
    assert_eq!(first.marks.marks, vec!["first_frame".to_owned()]);
    assert_eq!(first.marks.spans.len(), 1);
    assert_eq!(first.marks.spans[0].name, "render");

    // Entering the hover target changes the picture: the next frame carries the id.
    c.pointer_to(ID1, 150.0, 150.0);
    c.pump_until("hover frame", |s| buffer_commits(&s.commits).len() == 2);
    assert_eq!(buffer_commits(&c.state.commits)[1].marks.handled, vec![ID1]);

    // A motion inside the target changes nothing, so on-demand commits nothing.
    c.pointer_to(ID2, 160.0, 160.0);
    c.pump_for(Nanos::from_millis(100));
    assert_eq!(buffer_commits(&c.state.commits).len(), 2);

    // Leaving the target commits a frame that carries both ids.
    c.pointer_to(ID3, 10.0, 10.0);
    c.pump_until("leave frame", |s| buffer_commits(&s.commits).len() == 3);
    assert_eq!(
        buffer_commits(&c.state.commits)[2].marks.handled,
        vec![ID2, ID3]
    );

    assert!(c.close().success());
    let server_commits: Vec<Commit> = c.state.commits.clone();
    drop(c);

    let log = read_log(&path);
    let received: Vec<u64> = events(&log, "input_received")
        .iter()
        .filter_map(|e| e["marks_id"].as_u64())
        .collect();
    assert_eq!(received, vec![ID1, ID2, ID3]);
    let commits = events(&log, "commit");
    assert_eq!(commits.len(), 3);
    for (logged, server) in commits.iter().zip(buffer_commits(&server_commits)) {
        assert_eq!(logged["frame_id"].as_u64(), server.marks.frame);
        let span = &server.marks.spans[0];
        assert_eq!(logged["render_begin"].as_u64(), Some(span.begin.0));
        assert_eq!(logged["render_end"].as_u64(), Some(span.end.0));
        let handled: Vec<u64> = logged["handled"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert_eq!(handled, server.marks.handled);
        let t = logged["t"].as_u64().unwrap();
        assert!(span.begin.0 <= span.end.0 && span.end.0 <= t);
    }
    assert_eq!(commits[1]["state"]["hover"], true);
    assert_eq!(commits[2]["state"]["hover"], false);
    assert_eq!(events(&log, "close").len(), 1);
    let exit = log.last().unwrap();
    assert_eq!(exit["event"], "exit");
    assert_eq!(exit["code"], 0);
}

#[test]
fn input_delay_and_no_marks() {
    let path = log_path("delay");
    let args = [
        "--scene",
        "hover",
        "--mode",
        "on-demand",
        "--input-delay",
        "30ms",
    ];
    let mut c = TestCompositor::start(false, &path, &args);
    c.pump_until("first frame", |s| s.commits.iter().any(|c| c.buffer));
    c.pump_until("pointer", |s| !s.pointers.is_empty());
    c.pointer_to(0, 150.0, 150.0);
    c.pump_until("hover frame", |s| buffer_commits(&s.commits).len() == 2);
    assert!(c.close().success());
    drop(c);
    let log = read_log(&path);
    assert_eq!(log[1]["event"], "connected");
    assert_eq!(log[1]["marks"], false);
    let received = events(&log, "input_received");
    let applied = events(&log, "input_applied");
    assert_eq!(received.len(), 1);
    assert!(received[0]["marks_id"].is_null());
    let delay = applied[0]["t"].as_u64().unwrap() - received[0]["t"].as_u64().unwrap();
    assert!(delay >= 30_000_000, "applied after {delay} ns");
    let commit = events(&log, "commit")[1];
    assert!(commit["render_begin"].as_u64().unwrap() >= applied[0]["t"].as_u64().unwrap());
}

#[test]
fn frame_limit_and_render_delay() {
    let path = log_path("frames");
    let args = [
        "--scene",
        "animate",
        "--exit-after-frames",
        "6",
        "--render-delay",
        "4ms",
        "--jank-every",
        "3",
        "--jank",
        "6ms",
    ];
    let mut c = TestCompositor::start(true, &path, &args);
    let status = c.wait_exit();
    assert!(status.success());
    assert_eq!(buffer_commits(&c.state.commits).len(), 6);
    drop(c);
    let log = read_log(&path);
    for commit in events(&log, "commit") {
        let id = commit["frame_id"].as_u64().unwrap();
        let took =
            commit["render_end"].as_u64().unwrap() - commit["render_begin"].as_u64().unwrap();
        let want = if id % 3 == 0 { 10_000_000 } else { 4_000_000 };
        assert!(
            took >= want && took < want + 2_000_000,
            "frame {id} took {took} ns"
        );
    }
    assert_eq!(log.last().unwrap()["reason"], "frames done");
}

#[test]
fn fifo_and_commit_timing_on_every_commit() {
    let path = log_path("timing");
    let args = [
        "--scene",
        "animate",
        "--fifo",
        "--target-delay",
        "5ms",
        "--exit-after-frames",
        "4",
    ];
    let mut c = TestCompositor::start(true, &path, &args);
    assert!(c.wait_exit().success());
    let server = c.state.commits.clone();
    drop(c);
    let log = read_log(&path);
    let logged = events(&log, "commit");
    let buffers = buffer_commits(&server);
    assert_eq!(logged.len(), 4);
    for (l, s) in logged.iter().zip(buffers) {
        assert!(s.timing.wait_barrier && s.timing.set_barrier);
        assert_eq!(l["fifo"], true);
        let target = s.timing.target.expect("target").0;
        assert_eq!(l["target"].as_u64(), Some(target));
        let end = l["render_end"].as_u64().unwrap();
        assert!(target >= end + 5_000_000 && target <= l["t"].as_u64().unwrap() + 5_000_000);
    }
}

#[test]
fn missing_global_exits_with_code_4() {
    let path = log_path("missing");
    let mut c = TestCompositor::start(false, &path, &["--fifo"]);
    let status = c.wait_exit();
    assert_eq!(status.code(), Some(4));
    drop(c);
    let log = read_log(&path);
    assert_eq!(log.last().unwrap()["code"], 4);
}
