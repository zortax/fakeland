//! Tests of the recorder: unbounded and ring retention, captures and streaming cursors.

use fakeland_types::{ClientId, FakeClock, Nanos, OutputId, SurfaceId, Timestamp};

use crate::event::{Environment, Event, OutputInfo};
use crate::recorder::{CaptureId, Recorder, RecorderError, RecorderMode};
use crate::trace::TraceHeader;

fn header() -> TraceHeader {
    TraceHeader::new(Environment::default(), vec![], Timestamp(0))
}

fn mark(i: u64) -> Event {
    Event::UserMark {
        label: format!("m{i}"),
    }
}

fn labels(trace: &crate::trace::Trace) -> Vec<String> {
    trace
        .records
        .iter()
        .map(|r| match &r.event {
            Event::UserMark { label } => label.clone(),
            other => other.name().to_string(),
        })
        .collect()
}

#[test]
fn unbounded_keeps_everything() {
    let mut r = Recorder::new(header(), RecorderMode::Unbounded);
    for i in 0..100 {
        r.push(Timestamp(i), mark(i));
    }
    assert_eq!(r.len(), 100);
    assert_eq!(r.first_index(), 0);
    let s = r.snapshot();
    assert_eq!(s.records.len(), 100);
    assert_eq!(s.header.dropped, 0);
    assert_eq!(s.header.started, Timestamp(0));
}

#[test]
fn ring_keeps_the_last_records() {
    let mut r = Recorder::new(header(), RecorderMode::Ring { capacity: 3 });
    for i in 0..10 {
        r.push(Timestamp(i * 10), mark(i));
    }
    assert_eq!(r.len(), 3);
    assert_eq!(r.first_index(), 7);
    assert_eq!(r.end_index(), 10);
    let s = r.snapshot();
    assert_eq!(labels(&s), ["m7", "m8", "m9"]);
    assert_eq!(s.header.dropped, 7);
    assert_eq!(s.header.started, Timestamp(70));
}

#[test]
fn ring_keeps_context_of_live_entities() {
    let mut r = Recorder::new(header(), RecorderMode::Ring { capacity: 2 });
    r.push(
        Timestamp(1),
        Event::SurfaceCreated {
            surface: SurfaceId(1),
            client: ClientId(1),
        },
    );
    r.push(
        Timestamp(2),
        Event::SurfaceCreated {
            surface: SurfaceId(2),
            client: ClientId(1),
        },
    );
    r.push(
        Timestamp(3),
        Event::SurfaceDestroyed {
            surface: SurfaceId(2),
        },
    );
    for i in 0..5 {
        r.push(Timestamp(10 + i), mark(i));
    }
    let s = r.snapshot();
    assert_eq!(labels(&s), ["surface_created", "m3", "m4"]);
    assert_eq!(s.records[0].event.surface(), Some(SurfaceId(1)));
}

#[test]
fn ring_context_compacts() {
    let mut r = Recorder::new(header(), RecorderMode::Ring { capacity: 4 });
    for i in 0..10_000u64 {
        r.push(
            Timestamp(i * 2),
            Event::SurfaceCreated {
                surface: SurfaceId(i),
                client: ClientId(1),
            },
        );
        r.push(
            Timestamp(i * 2 + 1),
            Event::SurfaceDestroyed {
                surface: SurfaceId(i),
            },
        );
    }
    let s = r.snapshot();
    assert_eq!(s.records.len(), 4);
}

#[test]
fn capture_returns_the_span_with_context() {
    let out = OutputInfo {
        output: OutputId(0),
        name: "DP-1".into(),
        width: 1920,
        height: 1080,
        refresh_mhz: 60_000,
        scale: 1.0,
        transform: "normal".into(),
        timing: "fixed".into(),
    };
    let mut r = Recorder::new(header(), RecorderMode::Unbounded);
    r.push(Timestamp(1), Event::OutputConfigured(out.clone()));
    r.push(Timestamp(2), mark(0));
    let id = r.begin_capture();
    r.push(Timestamp(3), mark(1));
    r.push(Timestamp(4), mark(2));
    let trace = r.end_capture(id).unwrap();
    assert_eq!(labels(&trace), ["output_configured", "m1", "m2"]);
    assert_eq!(trace.header.started, Timestamp(3));
    assert_eq!(trace.header.outputs, vec![out]);
    assert!(matches!(
        r.end_capture(id),
        Err(RecorderError::UnknownCapture(CaptureId(0)))
    ));
}

#[test]
fn capture_in_ring_counts_dropped_records() {
    let mut r = Recorder::new(header(), RecorderMode::Ring { capacity: 2 });
    let id = r.begin_capture();
    for i in 0..5 {
        r.push(Timestamp(i), mark(i));
    }
    let trace = r.end_capture(id).unwrap();
    assert_eq!(labels(&trace), ["m3", "m4"]);
    assert_eq!(trace.header.dropped, 3);
}

#[test]
fn overlapping_captures() {
    let mut r = Recorder::new(header(), RecorderMode::Unbounded);
    let a = r.begin_capture();
    r.push(Timestamp(1), mark(0));
    let b = r.begin_capture();
    r.push(Timestamp(2), mark(1));
    assert_eq!(r.open_captures().count(), 2);
    let tb = r.end_capture(b).unwrap();
    r.push(Timestamp(3), mark(2));
    let ta = r.end_capture(a).unwrap();
    assert_eq!(labels(&tb), ["m1"]);
    assert_eq!(labels(&ta), ["m0", "m1", "m2"]);
}

#[test]
fn stream_cursor_and_take() {
    let clock = FakeClock::new(Timestamp(100));
    let mut r = Recorder::new(header(), RecorderMode::Unbounded);
    r.push_now(&clock, mark(0));
    let cursor = r.end_index();
    clock.advance(Nanos(5));
    r.push_now(&clock, mark(1));
    r.push_now(&clock, mark(2));
    let streamed: Vec<_> = r.since(cursor).map(|rec| rec.t).collect();
    assert_eq!(streamed, [Timestamp(105), Timestamp(105)]);
    let taken = r.take();
    assert_eq!(taken.records.len(), 3);
    assert!(r.is_empty());
    assert_eq!(r.first_index(), 3);
    r.push(Timestamp(200), mark(3));
    assert_eq!(r.since(0).count(), 1);
    assert_eq!(r.mode(), RecorderMode::Unbounded);
}
