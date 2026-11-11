//! Trace methods: captures of a span, snapshots, frame statistics and files.

use std::path::{Path, PathBuf};

use fakeland::WindowInfo;
use fakeland_scenario::WindowRef;
use fakeland_trace::TraceSummary;
use fakeland_types::Dur;
use serde::{Deserialize, Serialize};

use super::defaults;
use super::method::{NoParams, method};

/// The file format of an exported trace.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceFormat {
    /// The header, then one record per line.
    #[default]
    Jsonl,
    /// One JSON document.
    Json,
    /// Chrome trace JSON for ui.perfetto.dev.
    Perfetto,
    /// One CSV row per frame.
    Csv,
    /// The JSON summary of the trace.
    Summary,
}

impl TraceFormat {
    /// Picks a format from a file name: `.jsonl`, `.csv`, `.perfetto.json`, `.summary.json`,
    /// then `.json`. Other names give JSONL.
    pub fn from_path(path: &Path) -> Self {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if name.ends_with(".csv") {
            Self::Csv
        } else if name.ends_with(".perfetto.json") || name.ends_with(".pftrace.json") {
            Self::Perfetto
        } else if name.ends_with(".summary.json") {
            Self::Summary
        } else if name.ends_with(".json") {
            Self::Json
        } else {
            Self::Jsonl
        }
    }

    /// Returns the wire name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Jsonl => "jsonl",
            Self::Json => "json",
            Self::Perfetto => "perfetto",
            Self::Csv => "csv",
            Self::Summary => "summary",
        }
    }
}

/// Where a trace goes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceOutput {
    /// The absolute path on the server host.
    pub path: PathBuf,
    /// The format. `None` picks one from the file name.
    #[serde(default)]
    pub format: Option<TraceFormat>,
}

impl TraceOutput {
    /// Returns the format to write.
    pub fn format(&self) -> TraceFormat {
        self.format
            .unwrap_or_else(|| TraceFormat::from_path(&self.path))
    }
}

/// A trace capture in progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceCapture {
    /// The capture id.
    pub capture: u64,
}

/// The end of a trace capture.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TraceEndParams {
    /// The capture id. `None` ends the newest open capture of this server.
    #[serde(default)]
    pub capture: Option<u64>,
    /// The file to write. `None` returns only the summary.
    #[serde(default)]
    pub output: Option<TraceOutput>,
}

/// A trace of the recent past.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceSnapshotParams {
    /// Keeps only this last span. `None` keeps every buffered record.
    #[serde(default)]
    pub last: Option<Dur>,
    /// The file to write. `None` returns only the summary.
    #[serde(default)]
    pub output: Option<TraceOutput>,
}

/// A written or summarized trace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceFileInfo {
    /// The file, when one was written.
    pub path: Option<PathBuf>,
    /// The format of that file.
    pub format: Option<TraceFormat>,
    /// The count of records.
    pub records: usize,
    /// Records that fell out of the ring during the span.
    pub dropped: u64,
    /// The span of the records in milliseconds.
    pub duration_ms: f64,
    /// The trace summary.
    pub summary: TraceSummary,
}

/// A request for frame statistics of a window.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameStatsParams {
    /// The window. The default is the focused window.
    #[serde(default)]
    pub window: WindowRef,
    /// The span before now.
    #[serde(default = "defaults::stats_span")]
    pub last: Dur,
}

/// Frame statistics of one window over a recent span.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FrameStats {
    /// The window.
    pub window: WindowInfo,
    /// The span in milliseconds.
    pub span_ms: f64,
    /// The summary of the window's surfaces in that span.
    pub summary: TraceSummary,
    /// A short table for people.
    pub text: String,
}

method!(
    /// Starts a trace capture.
    TraceBegin, "trace_begin", NoParams => TraceCapture
);
method!(
    /// Ends a trace capture, writes the file and returns its summary.
    TraceEnd, "trace_end", TraceEndParams => TraceFileInfo
);
method!(
    /// Writes the buffered trace or its last span.
    TraceSnapshot, "trace_snapshot", TraceSnapshotParams => TraceFileInfo
);
method!(
    /// Summarizes the recent frames of a window.
    FrameStatsOf, "frame_stats", FrameStatsParams => FrameStats
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_formats_from_names() {
        let f = |p: &str| TraceFormat::from_path(Path::new(p));
        assert_eq!(f("/x/run.jsonl"), TraceFormat::Jsonl);
        assert_eq!(f("/x/run.csv"), TraceFormat::Csv);
        assert_eq!(f("/x/run.perfetto.json"), TraceFormat::Perfetto);
        assert_eq!(f("/x/run.summary.json"), TraceFormat::Summary);
        assert_eq!(f("/x/run.JSON"), TraceFormat::Json);
        assert_eq!(f("/x/run.trace"), TraceFormat::Jsonl);
    }
}
