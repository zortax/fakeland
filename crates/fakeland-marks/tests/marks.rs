//! End-to-end tests of fakeland-marks against an in-process compositor over a socket pair.

mod support;

use fakeland_marks::{
    InputEventId, InputKind, InputMark, MANAGER_INTERFACE, MarkError, Marks, Span, SurfaceError,
    Timestamp,
};
use fakeland_protocol::server::fakeland_marks_manager_v1::InputKind as ServerKind;
use support::app::App;
use support::record::{CommitMarks, Record, SpanRecord};
use support::server::TestServer;
use wayland_client::Proxy;

fn bound(marks: bool) -> (TestServer, App) {
    let (server, stream) = TestServer::start(marks);
    let app = App::connect(stream, Marks::bind);
    (server, app)
}

fn commit_marks(record: Record) -> CommitMarks {
    match record {
        Record::Commit { marks, .. } => marks,
        other => panic!("expected a commit, got {other:?}"),
    }
}

#[test]
fn input_id_is_ready_in_the_input_handler() {
    let (server, mut app) = bound(true);
    assert!(app.state.marks.is_active());

    let big = InputEventId((7 << 32) | 3);
    server.send_input(InputEventId(1), ServerKind::PointerMotion);
    server.send_input(big, ServerKind::PointerButton);
    app.roundtrip();

    assert_eq!(
        app.state.motions,
        vec![
            Some(InputMark {
                id: InputEventId(1),
                kind: Some(InputKind::PointerMotion)
            }),
            Some(InputMark {
                id: big,
                kind: Some(InputKind::PointerButton)
            }),
        ]
    );
    assert!(app.state.marks.input_events().is_empty());
}

#[test]
fn take_input_ids_returns_every_id_in_order() {
    let (server, mut app) = bound(true);
    app.state.take_in_handler = false;

    for id in 10..15 {
        server.send_input(InputEventId(id), ServerKind::Key);
    }
    app.roundtrip();

    assert_eq!(app.state.motions.len(), 5);
    let ids = app.state.marks.take_input_ids();
    assert_eq!(ids, (10..15).map(InputEventId).collect::<Vec<_>>());
    assert!(app.state.marks.take_input_ids().is_empty());
}

#[test]
fn surface_marks_apply_on_the_next_commit() {
    let (server, mut app) = bound(true);
    let surface = app.surface();
    let marks = app.state.marks.surface(&surface).expect("surface marks");
    assert!(marks.is_active());

    let (begin, end) = (Timestamp(3_000_000_123), Timestamp(7_500_000_000));
    app.state.marks.mark("boot").expect("client mark");
    marks.frame(u64::MAX - 1);
    marks.frame(41);
    marks.input_handled(InputEventId(7));
    marks.input_handled(InputEventId(1 << 40));
    marks.input_handled(InputEventId(7));
    marks.span("paint", begin, end).expect("span");
    Span::start_at("layout", begin)
        .finish_at(&marks, begin)
        .expect("span guard");
    marks.mark("ready").expect("mark");
    surface.commit();
    surface.commit();
    app.roundtrip();

    assert_eq!(server.next_record(), Record::ClientMark("boot".into()));
    let record = server.next_record();
    let Record::Commit { surface: id, .. } = &record else {
        panic!("expected a commit, got {record:?}");
    };
    assert_eq!(*id, surface.id().protocol_id());
    assert_eq!(
        commit_marks(record),
        CommitMarks {
            frame: Some(41),
            inputs: vec![InputEventId(7), InputEventId(1 << 40)],
            spans: vec![
                SpanRecord {
                    name: "paint".into(),
                    begin,
                    end
                },
                SpanRecord {
                    name: "layout".into(),
                    begin,
                    end: begin
                },
            ],
            marks: vec!["ready".into()],
        }
    );
    assert_eq!(commit_marks(server.next_record()), CommitMarks::default());
}

#[test]
fn span_guard_measures_on_the_monotonic_clock() {
    let (server, mut app) = bound(true);
    let surface = app.surface();
    let marks = app.state.marks.surface(&surface).expect("surface marks");

    let span = Span::start("gpu-submit");
    let begin = span.began_at();
    span.finish(&marks).expect("span");
    surface.commit();
    app.roundtrip();

    let spans = commit_marks(server.next_record()).spans;
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].name, "gpu-submit");
    assert_eq!(spans[0].begin, begin);
    assert!(spans[0].end >= begin);
}

#[test]
fn a_surface_has_one_marks_object() {
    let (server, mut app) = bound(true);
    let surface = app.surface();
    let first = app.state.marks.surface(&surface).expect("first");
    assert_eq!(
        app.state.marks.surface(&surface).err(),
        Some(SurfaceError::Taken)
    );
    drop(first);

    let second = app.state.marks.surface(&surface).expect("after drop");
    second.frame(5);
    surface.commit();
    app.roundtrip();
    assert_eq!(commit_marks(server.next_record()).frame, Some(5));
}

#[test]
fn dropped_marks_discard_pending_state() {
    let (server, mut app) = bound(true);
    let surface = app.surface();
    let marks = app.state.marks.surface(&surface).expect("marks");
    marks.frame(9);
    drop(marks);
    surface.commit();
    app.roundtrip();
    assert_eq!(commit_marks(server.next_record()), CommitMarks::default());
}

#[test]
fn marks_after_surface_destroy_do_nothing() {
    let (server, mut app) = bound(true);
    let surface = app.surface();
    let marks = app.state.marks.surface(&surface).expect("marks");
    surface.destroy();
    assert!(!marks.is_active());
    marks.frame(1);
    marks.input_handled(InputEventId(2));
    marks.mark("late").expect("mark");
    drop(marks);
    app.roundtrip();
    assert!(server.no_records());
    assert_eq!(
        app.state.marks.surface(&surface).err(),
        Some(SurfaceError::SurfaceDestroyed)
    );
}

#[test]
fn invalid_names_and_spans_never_reach_the_compositor() {
    let (server, mut app) = bound(true);
    let surface = app.surface();
    let marks = app.state.marks.surface(&surface).expect("marks");

    assert!(matches!(app.state.marks.mark(""), Err(MarkError::Name(_))));
    assert!(matches!(
        marks.mark(&"x".repeat(fakeland_marks::MAX_NAME_LEN + 1)),
        Err(MarkError::Name(_))
    ));
    assert!(matches!(
        marks.span("paint", Timestamp(10), Timestamp(9)),
        Err(MarkError::ReversedSpan { .. })
    ));
    surface.commit();
    app.roundtrip();
    assert_eq!(commit_marks(server.next_record()), CommitMarks::default());
}

#[test]
fn without_the_global_every_call_is_a_no_op() {
    let (server, mut app) = bound(false);
    assert!(!app.state.marks.is_active());

    let surface = app.surface();
    let marks = app.state.marks.surface(&surface).expect("inactive marks");
    assert!(!marks.is_active());
    let again = app.state.marks.surface(&surface).expect("no claim check");
    assert!(!again.is_active());

    app.state.marks.mark("boot").expect("mark");
    marks.frame(3);
    marks.input_handled(InputEventId(4));
    marks
        .span("paint", Timestamp(1), Timestamp(2))
        .expect("span");
    marks.mark("ready").expect("mark");
    assert!(marks.mark("").is_err());
    server.send_input(InputEventId(8), ServerKind::PointerMotion);
    surface.commit();
    app.roundtrip();

    assert_eq!(app.state.motions, vec![None]);
    assert!(app.state.marks.input_events().is_empty());
    assert!(app.state.marks.next_input().is_none());
    assert_eq!(commit_marks(server.next_record()), CommitMarks::default());
    assert!(server.no_records());
}

#[test]
fn discover_binds_on_a_private_registry() {
    let (server, stream) = TestServer::start(true);
    let mut app = App::connect(stream, |conn, _| Marks::discover(conn).expect("discover"));
    assert!(app.state.marks.is_active());
    server.send_input(InputEventId(21), ServerKind::Touch);
    app.roundtrip();
    assert_eq!(
        app.state.motions,
        vec![Some(InputMark {
            id: InputEventId(21),
            kind: Some(InputKind::Touch)
        })]
    );

    let (_server, stream) = TestServer::start(false);
    let app = App::connect(stream, |conn, _| Marks::discover(conn).expect("discover"));
    assert!(!app.state.marks.is_active());
}

#[test]
fn from_global_binds_a_global_that_the_app_found() {
    let (server, stream) = TestServer::start(true);
    let mut app = App::connect(stream, |conn, globals| {
        let global = globals
            .contents()
            .clone_list()
            .into_iter()
            .find(|g| g.interface == MANAGER_INTERFACE)
            .expect("marks global");
        Marks::from_global(conn, globals.registry(), global.name, global.version)
    });
    assert!(app.state.marks.is_active());
    server.send_input(InputEventId(5), ServerKind::Text);
    app.roundtrip();
    assert_eq!(app.state.motions[0].map(|m| m.id), Some(InputEventId(5)));

    let surface = app.surface();
    let marks = app.state.marks.surface(&surface).expect("marks");
    marks.frame(77);
    surface.commit();
    app.roundtrip();
    assert_eq!(commit_marks(server.next_record()).frame, Some(77));
    app.conn.flush().expect("flush");
}

#[test]
fn inactive_handle_needs_no_connection() {
    let mut marks = Marks::inactive();
    assert!(!marks.is_active());
    assert_eq!(marks.mark("x"), Ok(()));
    assert!(marks.take_input_ids().is_empty());
}

#[test]
fn handles_cross_threads() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Marks>();
    send_sync::<fakeland_marks::SurfaceMarks>();
    send_sync::<Span>();
}
