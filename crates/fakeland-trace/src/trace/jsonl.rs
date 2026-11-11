//! JSONL reading and writing of traces: the header on the first line, then one record per line.

use std::io::{BufRead, Write};

use crate::event::Record;
use crate::trace::{JsonlWriter, Trace, TraceError, TraceHeader};

impl Trace {
    /// Writes the trace as JSONL.
    pub fn write_jsonl(&self, w: impl Write) -> Result<(), TraceError> {
        let mut writer = JsonlWriter::new(w, &self.header)?;
        for r in &self.records {
            writer.write(r)?;
        }
        writer.flush()
    }

    /// Reads a JSONL trace and checks its format version. Blank lines are skipped.
    pub fn read_jsonl(r: impl BufRead) -> Result<Trace, TraceError> {
        let mut header: Option<TraceHeader> = None;
        let mut records = Vec::new();
        for (i, line) in r.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let json_err = |source| TraceError::Json {
                line: i + 1,
                source,
            };
            if header.is_none() {
                let h: TraceHeader = serde_json::from_str(&line).map_err(json_err)?;
                h.check_version()?;
                header = Some(h);
            } else {
                records.push(serde_json::from_str::<Record>(&line).map_err(json_err)?);
            }
        }
        let header = header.ok_or(TraceError::MissingHeader)?;
        Ok(Trace { header, records })
    }
}
