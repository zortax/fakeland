//! The Perfetto (Chrome JSON) export of the session trace, and a check that it holds the app
//! spans, the app frame ids and the input flows.

use std::collections::BTreeMap;
use std::fs::File;
use std::path::Path;

use fakeland_trace::frames::FrameTimeline;
use fakeland_trace::{Trace, write_chrome_trace};
use serde_json::Value;

/// What the export holds.
#[derive(Clone, Debug, Default)]
pub struct Contents {
    /// App span slices by name.
    pub app_spans: BTreeMap<String, usize>,
    /// Frame slices that carry an app frame id.
    pub app_frames: usize,
    /// Flow events by phase: `s` starts, `t` steps and `f` ends.
    pub flows: BTreeMap<String, usize>,
    /// App mark instants by name.
    pub app_marks: BTreeMap<String, usize>,
}

impl Contents {
    /// True when the export holds app spans, app frames and complete flows.
    pub fn ok(&self) -> bool {
        !self.app_spans.is_empty()
            && self.app_frames > 0
            && self.flows.get("s").copied().unwrap_or(0) > 0
            && self.flows.get("f").copied().unwrap_or(0) > 0
    }
}

/// Writes the export to `path` and reads it back.
pub fn write(
    trace: &Trace,
    timeline: &FrameTimeline,
    path: &Path,
) -> Result<Contents, Box<dyn std::error::Error>> {
    write_chrome_trace(trace, timeline, File::create(path)?)?;
    let doc: Value = serde_json::from_reader(std::io::BufReader::new(File::open(path)?))?;
    let mut c = Contents::default();
    let events = doc["traceEvents"].as_array().cloned().unwrap_or_default();
    for e in &events {
        let ph = e["ph"].as_str().unwrap_or("");
        let cat = e["cat"].as_str().unwrap_or("");
        let name = e["name"].as_str().unwrap_or("").to_owned();
        match (ph, cat) {
            ("X", "app") => *c.app_spans.entry(name).or_default() += 1,
            ("i" | "I", "app") => *c.app_marks.entry(name).or_default() += 1,
            ("s" | "t" | "f", _) => *c.flows.entry(ph.to_owned()).or_default() += 1,
            _ => {}
        }
        if ph == "X" && !e["args"]["app_frame_id"].is_null() {
            c.app_frames += 1;
        }
    }
    println!("\n== perfetto export {}", path.display());
    println!(
        "  events {}  frame slices with app_frame_id {}",
        events.len(),
        c.app_frames
    );
    println!("  app spans {:?}", c.app_spans);
    println!("  app marks {:?}", c.app_marks);
    println!("  flow events {:?}", c.flows);
    Ok(c)
}
