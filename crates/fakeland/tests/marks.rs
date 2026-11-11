//! The app-mark server against the reference client: frame ids, render spans and named marks
//! attach to the commits they came with, input ids reach the client before their events, and the
//! global stays hidden without opt-in. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use std::collections::HashMap;

use fakeland::scenario::Position;
use fakeland::{Event, Fakeland, LaunchSpec, OutputConfig, Session, WindowRef};
use fakeland_trace::{AppMarkKind, FrameTimeline};
use fakeland_types::{CommitId, Hz, Nanos, Timestamp};

fn session(marks: bool) -> Session {
    Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .app_marks(marks)
        .start()
        .expect("start")
}

#[test]
#[ignore = "runs the reference client"]
fn marks_attach_to_their_commits() {
    let _serial = support::serial();
    let log_file = support::log_path("marks");
    let s = session(true);
    let app = s
        .blocking_launch(LaunchSpec::new(support::testclient()).args([
            "--scene",
            "hover",
            "--mode",
            "callback",
            "--render-delay",
            "2ms",
            "--app-id",
            "marks",
            "--exit-after",
            "30",
            "--log-json",
            log_file.to_str().unwrap(),
        ]))
        .expect("launch");
    let w = s
        .blocking_wait_window(WindowRef::app_id("marks"), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let me = WindowRef::id(w.id);
    s.blocking_wait_frames(me.clone(), 10, Nanos::from_secs(5))
        .expect("frames");
    let sent = s
        .blocking_pointer_move(Position::window(me.clone(), 200.0, 175.0))
        .expect("move");
    s.blocking_wait_frames(me.clone(), 20, Nanos::from_secs(5))
        .expect("frames");
    let trace = s.blocking_trace_snapshot().expect("trace");
    let log = support::read_log(&log_file);

    let mut frame_of: HashMap<CommitId, u64> = HashMap::new();
    let mut spans: HashMap<CommitId, (Timestamp, Timestamp)> = HashMap::new();
    let mut handled: HashMap<CommitId, Vec<u64>> = HashMap::new();
    let mut named: Vec<(Option<CommitId>, String)> = Vec::new();
    for r in &trace.records {
        if let Event::AppMark { commit, mark, .. } = &r.event {
            match (commit, mark) {
                (Some(c), AppMarkKind::Frame { frame_id }) => {
                    frame_of.insert(*c, *frame_id);
                }
                (Some(c), AppMarkKind::Span { name, begin, end }) if name == "render" => {
                    spans.insert(*c, (*begin, *end));
                }
                (Some(c), AppMarkKind::InputHandled { input }) => {
                    handled.entry(*c).or_default().push(input.0)
                }
                (c, AppMarkKind::Mark { name }) => named.push((*c, name.clone())),
                _ => {}
            }
        }
    }
    let commit_time: HashMap<CommitId, Timestamp> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::Commit { commit, .. } => Some((*commit, r.t)),
            _ => None,
        })
        .collect();
    let by_frame: HashMap<u64, CommitId> = frame_of.iter().map(|(c, f)| (*f, *c)).collect();
    let client_commits = support::log_events(&log, "commit");
    assert!(client_commits.len() >= 25, "{}", client_commits.len());
    let mut lags = Vec::new();
    for c in &client_commits {
        let fid = c["frame_id"].as_u64().unwrap();
        let commit = *by_frame
            .get(&fid)
            .unwrap_or_else(|| panic!("frame {fid} has a frame mark"));
        let (begin, end) = spans[&commit];
        assert_eq!(
            begin.0,
            c["render_begin"].as_u64().unwrap(),
            "frame {fid} span begin"
        );
        assert_eq!(
            end.0,
            c["render_end"].as_u64().unwrap(),
            "frame {fid} span end"
        );
        let ids: Vec<u64> = c["handled"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_u64())
            .collect();
        assert_eq!(
            handled.get(&commit).cloned().unwrap_or_default(),
            ids,
            "frame {fid}"
        );
        let lag = commit_time[&commit].0 as i64 - c["t"].as_u64().unwrap() as i64;
        assert!(
            lag >= 0,
            "frame {fid}: the compositor commit follows the client commit"
        );
        lags.push(lag);
    }
    support::summarize("client commit -> compositor commit record", &lags);
    assert!(
        named.contains(&(None, "connected".to_string()))
            && named.contains(&(None, "configured".to_string()))
    );
    let first = named
        .iter()
        .find(|(_, n)| n == "first_frame")
        .and_then(|(c, _)| *c)
        .expect("first_frame on a commit");
    assert_eq!(frame_of.get(&first), Some(&1));
    let id = sent[0].id;
    let got = support::log_events(&log, "input_received")
        .into_iter()
        .find(|l| l["kind"] == "pointer_motion" && l["marks_id"].as_u64() == Some(id.0));
    assert!(
        got.is_some(),
        "the client saw input id {id} with its motion"
    );
    assert!(handled.values().any(|v| v.contains(&id.0)));
    let timeline = FrameTimeline::build(&trace);
    let marked = timeline
        .frames(w.surface)
        .iter()
        .filter(|f| f.marks.frame_id.is_some())
        .count();
    assert!(marked >= 25);
}

#[test]
#[ignore = "runs the reference client"]
fn the_marks_global_needs_opt_in() {
    let _serial = support::serial();
    let log_file = support::log_path("no-marks");
    let s = session(false);
    let app = s
        .blocking_launch(LaunchSpec::new(support::testclient()).args([
            "--app-id",
            "plain",
            "--exit-after",
            "10",
            "--log-json",
            log_file.to_str().unwrap(),
        ]))
        .expect("launch");
    s.blocking_wait_window(WindowRef::app_id("plain"), Nanos::from_secs(10))
        .unwrap_or_else(|e| panic!("no window ({e}): {}", app.stderr()));
    let log = support::read_log(&log_file);
    let connected = support::log_events(&log, "connected");
    assert_eq!(connected[0]["marks"], false);
    assert!(
        !connected[0]["globals"]
            .as_array()
            .unwrap()
            .iter()
            .any(|g| g["interface"].as_str().unwrap_or("").contains("fakeland"))
    );
}
