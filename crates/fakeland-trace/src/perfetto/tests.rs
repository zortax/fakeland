//! Tests of the Chrome trace export: valid JSON with the expected processes, slices, flows,
//! counters and instants.

use serde_json::Value;

use fakeland_types::{InputEventId, Nanos};

use crate::event::{AppMarkKind, InputKind};
use crate::frames::FrameTimeline;
use crate::perfetto::write_chrome_trace;
use crate::testing::{every_event_trace, steady, vblank_time};
use crate::trace::Trace;

fn export(trace: &Trace) -> Vec<Value> {
    let tl = FrameTimeline::build(trace);
    let mut out = Vec::new();
    write_chrome_trace(trace, &tl, &mut out).unwrap();
    let doc: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(doc["displayTimeUnit"], "ms");
    doc["traceEvents"].as_array().cloned().unwrap()
}

fn named<'a>(events: &'a [Value], ph: &str, name: &str) -> Vec<&'a Value> {
    events
        .iter()
        .filter(|e| e["ph"] == ph && e["name"] == name)
        .collect()
}

fn thread_names(events: &[Value]) -> Vec<String> {
    events
        .iter()
        .filter(|e| e["ph"] == "M" && e["name"] == "thread_name")
        .filter_map(|e| e["args"]["name"].as_str().map(str::to_string))
        .collect()
}

fn trace_with_input() -> Trace {
    let mut f = steady(6);
    let t = vblank_time(2) + Nanos::from_millis(1);
    f.input(t, 9, 1, InputKind::PointerButton);
    f.mark(
        t,
        Some(1),
        None,
        AppMarkKind::InputHandled {
            input: InputEventId(9),
        },
    );
    f.mark(
        t,
        Some(1),
        None,
        AppMarkKind::Span {
            name: "layout".into(),
            begin: t,
            end: t + Nanos::from_millis(2),
        },
    );
    let mut trace = f.build();
    trace.records.sort_by_key(|r| r.t);
    trace
}

#[test]
fn processes_tracks_and_stage_slices() {
    let events = export(&trace_with_input());
    let process_names: Vec<&str> = events
        .iter()
        .filter(|e| e["ph"] == "M" && e["name"] == "process_name")
        .filter_map(|e| e["args"]["name"].as_str())
        .collect();
    assert!(process_names.contains(&"compositor"));
    assert!(process_names.contains(&"org.test.app (pid 4242)"));
    let threads = thread_names(&events);
    for t in [
        "surface#1 xdg_toplevel frames",
        "surface#1 xdg_toplevel client",
        "surface#1 xdg_toplevel gpu",
        "surface#1 xdg_toplevel queue",
        "surface#1 xdg_toplevel display",
        "surface#1 xdg_toplevel on screen",
        "surface#1 xdg_toplevel app spans",
        "DP-1 vblank",
        "input",
        "input handling",
    ] {
        assert!(threads.iter().any(|n| n == t), "missing thread {t}");
    }
    for stage in ["client", "gpu", "queue", "display"] {
        assert_eq!(named(&events, "X", stage).len(), 6, "stage {stage}");
    }
    let gpu = named(&events, "X", "gpu")[0];
    assert!((gpu["dur"].as_f64().unwrap() - 2000.0).abs() < 1e-6);
    assert_eq!(named(&events, "X", "commit#1").len(), 2);
    assert_eq!(named(&events, "X", "layout").len(), 1);
    assert_eq!(named(&events, "i", "vblank").len(), 7);
}

#[test]
fn input_flows_through_commit_to_presentation() {
    let events = export(&trace_with_input());
    let flows: Vec<&Value> = events
        .iter()
        .filter(|e| e["cat"] == "input" && e["id"] == 9)
        .collect();
    let phases: Vec<&str> = flows.iter().filter_map(|e| e["ph"].as_str()).collect();
    assert_eq!(phases, ["s", "t", "f"]);
    assert_eq!(flows[2]["bp"], "e");
    // Each flow point lies inside a complete slice on its thread.
    for p in &flows {
        let ts = p["ts"].as_f64().unwrap();
        let inside = events.iter().any(|e| {
            e["ph"] == "X"
                && e["tid"] == p["tid"]
                && e["pid"] == p["pid"]
                && e["ts"].as_f64().unwrap() <= ts
                && ts <= e["ts"].as_f64().unwrap() + e["dur"].as_f64().unwrap()
        });
        assert!(inside, "flow point {p} binds to no slice");
    }
    let instants = named(&events, "i", "pointer_button");
    assert_eq!(instants.len(), 1);
    // The mark arrives before commit 3, so commit 3 handles the input.
    assert_eq!(instants[0]["args"]["handled_by"], 3);
}

#[test]
fn counters_for_rate_damage_and_samples() {
    let events = export(&trace_with_input());
    let fps = named(&events, "C", "surface#1 fps");
    assert_eq!(fps.len(), 5);
    assert!((fps[0]["args"]["value"].as_f64().unwrap() - 60.0).abs() < 1e-3);
    assert_eq!(named(&events, "C", "surface#1 damage px").len(), 6);
    assert_eq!(named(&events, "C", "DP-1 wake lateness us").len(), 7);
    let all = export(&every_event_trace());
    assert_eq!(named(&all, "C", "cpu %").len(), 1);
    assert_eq!(named(&all, "C", "rss MiB")[0]["args"]["value"], 1024.0);
    assert_eq!(named(&all, "C", "gpu %").len(), 1);
    assert_eq!(named(&all, "C", "pss MiB")[0]["args"]["value"], 512.0);
    assert_eq!(
        named(&all, "C", "gpu proc mem MiB")[0]["args"]["value"],
        300.0
    );
}

#[test]
fn every_event_exports() {
    let events = export(&every_event_trace());
    assert!(!named(&events, "X", "scroll").is_empty());
    assert!(!named(&events, "X", "pointer_move").is_empty());
    assert!(!named(&events, "X", "readback").is_empty());
    assert!(!named(&events, "X", "output DP-1").is_empty());
    assert!(!named(&events, "i", "glide").is_empty());
    assert!(!named(&events, "i", "configure 5").is_empty());
    assert!(!named(&events, "i", "dropped vblank").is_empty());
    let user = named(&events, "i", "start, scroll");
    assert_eq!(user[0]["s"], "g");
    assert!(
        events
            .iter()
            .all(|e| e["ts"].as_f64().is_some_and(|t| t >= 0.0))
    );
}

#[test]
fn empty_trace_exports() {
    let trace = Trace::new(crate::trace::TraceHeader::new(
        crate::event::Environment::default(),
        vec![],
        fakeland_types::Timestamp(5),
    ));
    let events = export(&trace);
    assert_eq!(named(&events, "M", "process_name").len(), 1);
}

#[test]
fn meter_rates_from_cumulative_samples() {
    use crate::event::Event;
    use fakeland_types::ProcessId;
    let mut trace = every_event_trace();
    let t = trace.end();
    for (k, (cpu, switches, uj, busy)) in [
        (0u64, 0u64, 0u64, 0u64),
        (50_000_000, 100, 2_000_000, 25_000_000),
    ]
    .into_iter()
    .enumerate()
    {
        let at = t + Nanos::from_millis(100 * (k as i64 + 1));
        trace.push(
            at,
            Event::ProcessDetail {
                pid: ProcessId(9),
                cpu_ns_total: cpu,
                ctx_switches_total: switches,
                pss_bytes: None,
                private_bytes: None,
                swap_bytes: None,
                hwm_bytes: None,
            },
        );
        trace.push(
            at,
            Event::EnergySample {
                domain: "gpu-nvml-0".into(),
                microjoules_total: uj,
            },
        );
        trace.push(
            at,
            Event::GpuProcessSample {
                pid: ProcessId(9),
                mem_bytes: None,
                busy_ns_total: Some(busy),
                util_percent: None,
                source: "fdinfo:i915".into(),
            },
        );
    }
    let events = export(&trace);
    let cpu = named(&events, "C", "cpu ms/s");
    assert!((cpu[0]["args"]["value"].as_f64().unwrap() - 500.0).abs() < 1e-6);
    let wake = named(&events, "C", "wakeups/s");
    assert!((wake[0]["args"]["value"].as_f64().unwrap() - 1000.0).abs() < 1e-6);
    let watts = named(&events, "C", "gpu-nvml-0 W");
    assert!((watts[0]["args"]["value"].as_f64().unwrap() - 20.0).abs() < 1e-6);
    let busy = named(&events, "C", "gpu proc %");
    assert!((busy[0]["args"]["value"].as_f64().unwrap() - 25.0).abs() < 1e-6);
}
