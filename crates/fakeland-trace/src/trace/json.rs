//! Whole-document JSON reading and writing of traces.

use std::io::{Read, Write};

use crate::trace::{Trace, TraceError};

impl Trace {
    /// Writes the trace as one JSON document.
    pub fn write_json(&self, w: impl Write) -> Result<(), TraceError> {
        serde_json::to_writer(w, self).map_err(|source| TraceError::Json { line: 0, source })
    }

    /// Reads a trace from one JSON document and checks its format version.
    pub fn read_json(r: impl Read) -> Result<Trace, TraceError> {
        let trace: Trace =
            serde_json::from_reader(r).map_err(|source| TraceError::Json { line: 0, source })?;
        trace.header.check_version()?;
        Ok(trace)
    }
}
