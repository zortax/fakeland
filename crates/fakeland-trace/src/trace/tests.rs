//! Serialization tests: JSONL and JSON round trips of every event variant, version checks and
//! forward compatibility.

use std::io::BufReader;

use fakeland_types::Timestamp;

use crate::event::{Event, Record, TRACE_FORMAT_VERSION};
use crate::testing::{every_event, every_event_trace};
use crate::trace::{JsonlWriter, Trace, TraceError};

#[test]
fn jsonl_round_trip_of_every_variant() {
    let trace = every_event_trace();
    let mut out = Vec::new();
    trace.write_jsonl(&mut out).unwrap();
    let text = String::from_utf8(out.clone()).unwrap();
    assert_eq!(text.lines().count(), trace.records.len() + 1);
    let back = Trace::read_jsonl(BufReader::new(out.as_slice())).unwrap();
    assert_eq!(back, trace);
}

#[test]
fn json_round_trip_of_every_variant() {
    let trace = every_event_trace();
    let mut out = Vec::new();
    trace.write_json(&mut out).unwrap();
    let back = Trace::read_json(out.as_slice()).unwrap();
    assert_eq!(back, trace);
}

#[test]
fn every_variant_has_a_distinct_tag() {
    let mut names: Vec<&str> = every_event().iter().map(Event::name).collect();
    let n = names.len();
    names.sort_unstable();
    names.dedup();
    // Four app marks share one tag.
    assert_eq!(names.len(), n - 3);
}

#[test]
fn records_are_flat_and_tagged() {
    let r = Record::new(Timestamp(5), Event::UserMark { label: "x".into() });
    let json = serde_json::to_string(&r).unwrap();
    assert_eq!(json, r#"{"t":5,"type":"user_mark","label":"x"}"#);
}

#[test]
fn unknown_types_read_as_unknown() {
    let r: Record = serde_json::from_str(r#"{"t":5,"type":"from_the_future","x":1}"#).unwrap();
    assert_eq!(r.event, Event::Unknown);
}

#[test]
fn version_and_header_errors() {
    let mut trace = every_event_trace();
    trace.header.format_version = TRACE_FORMAT_VERSION + 1;
    let mut out = Vec::new();
    trace.write_jsonl(&mut out).unwrap();
    assert!(matches!(
        Trace::read_jsonl(out.as_slice()),
        Err(TraceError::Version { .. })
    ));
    assert!(matches!(
        Trace::read_jsonl(&b"\n\n"[..]),
        Err(TraceError::MissingHeader)
    ));
    let bad = format!(
        "{}\nnot json\n",
        serde_json::to_string(&every_event_trace().header).unwrap()
    );
    assert!(matches!(
        Trace::read_jsonl(bad.as_bytes()),
        Err(TraceError::Json { line: 2, .. })
    ));
}

#[test]
fn streaming_writer_matches_write_jsonl() {
    let trace = every_event_trace();
    let mut w = JsonlWriter::new(Vec::new(), &trace.header).unwrap();
    for r in &trace.records {
        w.write(r).unwrap();
    }
    assert_eq!(w.written(), trace.records.len() as u64);
    let streamed = w.into_inner();
    let mut direct = Vec::new();
    trace.write_jsonl(&mut direct).unwrap();
    assert_eq!(streamed, direct);
}
