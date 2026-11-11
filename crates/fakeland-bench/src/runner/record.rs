//! The data of one iteration.

use crate::input::{AnalysisInput, IterationMeta};
use crate::report::IterationSummary;
use crate::validity::LatenessStats;

/// One iteration: its metadata, outcome and analysis input.
#[derive(Clone, Debug)]
pub struct IterationRecord {
    /// The metadata.
    pub meta: IterationMeta,
    /// The driver error.
    pub error: Option<String>,
    /// The output wake lateness of the iteration.
    pub lateness: LatenessStats,
    /// The trace, timeline and side results. Warmups drop theirs unless the bench keeps them.
    pub input: Option<AnalysisInput>,
}

impl IterationRecord {
    /// Returns true when the driver finished without an error.
    pub fn ok(&self) -> bool {
        self.error.is_none()
    }

    /// Returns the report summary of the iteration.
    pub fn summary(&self) -> IterationSummary {
        IterationSummary {
            index: self.meta.index,
            measured: self.meta.measured,
            variant: self.meta.variant.clone(),
            phase: self.meta.phase,
            started: self.meta.started,
            finished: self.meta.finished,
            ok: self.ok(),
            error: self.error.clone(),
            late_wakes: self.lateness.late,
            max_lateness: self.lateness.max,
            records: self.input.as_ref().map_or(0, |i| i.trace.records.len()),
            dropped_records: self.input.as_ref().map_or(0, |i| i.trace.header.dropped),
        }
    }
}
