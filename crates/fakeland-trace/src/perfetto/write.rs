//! The export entry point: runs every emitter and writes the JSON document.

use std::io::{BufWriter, Write};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::frames::FrameTimeline;
use crate::perfetto::chrome::ChromeEvent;
use crate::perfetto::emitter::Emitter;
use crate::perfetto::{PerfettoError, compositor, counters, frames, inputs};
use crate::trace::Trace;

#[derive(Serialize)]
struct Document<'a> {
    #[serde(rename = "traceEvents")]
    trace_events: Vec<&'a ChromeEvent>,
    #[serde(rename = "displayTimeUnit")]
    display_time_unit: &'static str,
    #[serde(rename = "otherData")]
    other_data: Map<String, Value>,
}

/// Writes a trace and its timeline as Chrome Trace Event JSON.
///
/// Times are microseconds since the trace start. Each client is a process named by its app id
/// and pid; the compositor is its own process. Each surface has tracks for whole frames, the
/// client, gpu, queue and display stages, the time on screen, app spans and app marks. Flow
/// arrows run from each input event through the commit that handles it to the presentation that
/// shows it. Counter tracks hold frame rate, damage, process and GPU samples and wake lateness.
pub fn write_chrome_trace(
    trace: &Trace,
    timeline: &FrameTimeline,
    w: impl Write,
) -> Result<(), PerfettoError> {
    let mut em = Emitter::new(trace, timeline);
    frames::emit(&mut em);
    inputs::emit(&mut em);
    counters::emit(&mut em);
    compositor::emit(&mut em);
    let env = &trace.header.environment;
    let mut other = Map::new();
    other.insert("format_version".into(), trace.header.format_version.into());
    other.insert("started_ns".into(), trace.header.started.as_nanos().into());
    other.insert("gpu".into(), env.gpu.clone().into());
    other.insert("driver".into(), env.driver.clone().into());
    other.insert("kernel".into(), env.kernel.clone().into());
    other.insert("renderer".into(), env.renderer.clone().into());
    other.insert(
        "fakeland_version".into(),
        env.fakeland_version.clone().into(),
    );
    let doc = Document {
        trace_events: em.tracks.metadata.iter().chain(&em.events).collect(),
        display_time_unit: "ms",
        other_data: other,
    };
    let mut out = BufWriter::new(w);
    serde_json::to_writer(&mut out, &doc)?;
    out.flush()?;
    Ok(())
}
