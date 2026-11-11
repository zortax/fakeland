//! Writes a trace as JSONL, JSON, Chrome trace, CSV or summary, and summarizes it.

use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use fakeland_trace::{FrameTimeline, Trace, TraceSummary};
use fakeland_types::Nanos;
use thiserror::Error;

use crate::methods::{TraceFileInfo, TraceFormat, TraceOutput};

/// A trace export that failed.
#[derive(Debug, Error)]
pub enum ExportError {
    /// The file cannot be written.
    #[error("{path}: {error}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The error.
        error: std::io::Error,
    },
    /// The exporter failed.
    #[error("{path}: {message}")]
    Format {
        /// The file.
        path: PathBuf,
        /// The error text.
        message: String,
    },
}

/// Summarizes a trace without writing a file.
pub fn trace_info(trace: &Trace) -> TraceFileInfo {
    let timeline = FrameTimeline::build(trace);
    info(trace, &timeline, None)
}

/// Returns the span of a trace from its header start to its last record. A slice or capture
/// starts with older context records, so the header start marks its real beginning.
pub fn span_ms(trace: &Trace) -> f64 {
    let start = trace.header.started.max(trace.start());
    trace.end().since(start).max(Nanos::ZERO).as_millis_f64()
}

fn summarize(trace: &Trace, timeline: &FrameTimeline) -> TraceSummary {
    let mut summary = TraceSummary::build(trace, timeline);
    summary.duration_ms = span_ms(trace);
    summary
}

fn info(
    trace: &Trace,
    timeline: &FrameTimeline,
    out: Option<(&Path, TraceFormat)>,
) -> TraceFileInfo {
    TraceFileInfo {
        path: out.map(|(p, _)| p.to_path_buf()),
        format: out.map(|(_, f)| f),
        records: trace.records.len(),
        dropped: trace.header.dropped,
        duration_ms: span_ms(trace),
        summary: summarize(trace, timeline),
    }
}

/// Writes a trace to a file in the format of the output and returns its summary. The directory
/// of the file is created.
pub fn export_trace(trace: &Trace, output: &TraceOutput) -> Result<TraceFileInfo, ExportError> {
    let path = output.path.as_path();
    let format = output.format();
    let io = |error| ExportError::Io {
        path: path.to_path_buf(),
        error,
    };
    let fmt = |message: String| ExportError::Format {
        path: path.to_path_buf(),
        message,
    };
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(io)?;
    }
    let timeline = FrameTimeline::build(trace);
    let file = File::create(path).map_err(io)?;
    let mut w = BufWriter::new(file);
    match format {
        TraceFormat::Jsonl => trace.write_jsonl(&mut w).map_err(|e| fmt(e.to_string()))?,
        TraceFormat::Json => trace.write_json(&mut w).map_err(|e| fmt(e.to_string()))?,
        TraceFormat::Perfetto => fakeland_trace::write_chrome_trace(trace, &timeline, &mut w)
            .map_err(|e| fmt(e.to_string()))?,
        TraceFormat::Csv => fakeland_trace::csv::write_frames(&timeline, &mut w).map_err(io)?,
        TraceFormat::Summary => summarize(trace, &timeline)
            .write_json(&mut w)
            .map_err(|e| fmt(e.to_string()))?,
    }
    w.flush().map_err(io)?;
    Ok(info(trace, &timeline, Some((path, format))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_trace::{Environment, Event, TraceHeader};
    use fakeland_types::Timestamp;

    fn sample() -> Trace {
        let mut trace = Trace::new(TraceHeader::new(
            Environment::default(),
            Vec::new(),
            Timestamp(1_000),
        ));
        trace.push(Timestamp(1_000), Event::UserMark { label: "a".into() });
        trace.push(Timestamp(3_001_000), Event::UserMark { label: "b".into() });
        trace
    }

    #[test]
    fn spans_start_at_the_header() {
        let mut trace = sample();
        assert!((span_ms(&trace) - 3.0).abs() < 1e-9);
        trace.header.started = Timestamp(2_001_000);
        assert!((span_ms(&trace) - 1.0).abs() < 1e-9);
        assert!((trace_info(&trace).summary.duration_ms - 1.0).abs() < 1e-9);
    }

    #[test]
    fn writes_every_format() {
        let dir = tempfile::tempdir().unwrap();
        let trace = sample();
        for (name, format) in [
            ("t.jsonl", TraceFormat::Jsonl),
            ("t.json", TraceFormat::Json),
            ("t.perfetto.json", TraceFormat::Perfetto),
            ("t.csv", TraceFormat::Csv),
            ("t.summary.json", TraceFormat::Summary),
        ] {
            let path = dir.path().join("sub").join(name);
            let out = TraceOutput {
                path: path.clone(),
                format: None,
            };
            let info = export_trace(&trace, &out).unwrap();
            assert_eq!(info.format, Some(format));
            assert_eq!(info.records, 2);
            assert!((info.duration_ms - 3.0).abs() < 1e-9);
            assert!(std::fs::metadata(&path).unwrap().len() > 0, "{name}");
        }
        let back = crate::export::read_trace(&dir.path().join("sub/t.jsonl")).unwrap();
        assert_eq!(back.records, trace.records);
        let back = crate::export::read_trace(&dir.path().join("sub/t.json")).unwrap();
        assert_eq!(back.records, trace.records);
    }
}
