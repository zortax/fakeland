//! What the test compositor observed: client-level marks and the marks applied on each commit.

use fakeland_types::{InputEventId, Timestamp};

/// One client-side phase as the compositor decoded it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpanRecord {
    pub name: String,
    pub begin: Timestamp,
    pub end: Timestamp,
}

/// The double-buffered marks of a surface.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CommitMarks {
    pub frame: Option<u64>,
    pub inputs: Vec<InputEventId>,
    pub spans: Vec<SpanRecord>,
    pub marks: Vec<String>,
}

/// One observation of the compositor.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Record {
    /// A client-level mark.
    ClientMark(String),
    /// A wl_surface.commit with the marks that it applied.
    Commit { surface: u32, marks: CommitMarks },
}
