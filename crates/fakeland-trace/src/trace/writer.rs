//! A streaming JSONL trace writer for long sessions and ring-buffer streaming.

use std::io::Write;

use crate::event::Record;
use crate::trace::{TraceError, TraceHeader};

/// Writes a header line, then one line per record.
#[derive(Debug)]
pub struct JsonlWriter<W: Write> {
    out: W,
    written: u64,
}

impl<W: Write> JsonlWriter<W> {
    /// Writes the header line and returns the writer.
    pub fn new(mut out: W, header: &TraceHeader) -> Result<Self, TraceError> {
        line(&mut out, header)?;
        Ok(Self { out, written: 0 })
    }

    /// Writes one record line.
    pub fn write(&mut self, record: &Record) -> Result<(), TraceError> {
        line(&mut self.out, record)?;
        self.written += 1;
        Ok(())
    }

    /// Returns the number of record lines written.
    pub const fn written(&self) -> u64 {
        self.written
    }

    /// Flushes the output.
    pub fn flush(&mut self) -> Result<(), TraceError> {
        self.out.flush()?;
        Ok(())
    }

    /// Returns the output.
    pub fn into_inner(self) -> W {
        self.out
    }
}

fn line(out: &mut impl Write, value: &impl serde::Serialize) -> Result<(), TraceError> {
    serde_json::to_writer(&mut *out, value)
        .map_err(|source| TraceError::Json { line: 0, source })?;
    out.write_all(b"\n")?;
    Ok(())
}
