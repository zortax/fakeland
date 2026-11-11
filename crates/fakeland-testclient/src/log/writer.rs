//! The JSON lines writer. It writes one object per event, flushes after every line so a killed
//! client loses nothing, and stops writing after the first write error.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use fakeland_types::Timestamp;
use serde::Serialize;

use super::LogEvent;

/// Where the log goes.
pub struct EventLog {
    out: Option<Box<dyn Write>>,
}

impl std::fmt::Debug for EventLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EventLog")
            .field("enabled", &self.out.is_some())
            .finish()
    }
}

#[derive(Serialize)]
struct Line<'a> {
    t: u64,
    #[serde(flatten)]
    event: &'a LogEvent,
}

impl EventLog {
    /// Builds a log that writes nothing.
    pub fn disabled() -> Self {
        Self { out: None }
    }

    /// Builds a log over a writer.
    pub fn to_writer(w: impl Write + 'static) -> Self {
        Self {
            out: Some(Box::new(w)),
        }
    }

    /// Opens the log target: `-` is stdout, anything else a file that is created or truncated.
    pub fn open(target: &Path) -> io::Result<Self> {
        if target == Path::new("-") {
            return Ok(Self::to_writer(io::stdout()));
        }
        let file = File::create(target)?;
        Ok(Self::to_writer(BufWriter::new(file)))
    }

    /// Writes one event at a time.
    pub fn write(&mut self, t: Timestamp, event: &LogEvent) {
        let Some(out) = self.out.as_mut() else {
            return;
        };
        let line = Line { t: t.0, event };
        let result = serde_json::to_writer(&mut *out, &line)
            .map_err(io::Error::from)
            .and_then(|()| out.write_all(b"\n"))
            .and_then(|()| out.flush());
        if let Err(e) = result {
            eprintln!("fakeland-testclient: event log disabled: {e}");
            self.out = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    #[derive(Clone, Default)]
    struct Shared(Rc<RefCell<Vec<u8>>>);

    impl Write for Shared {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn writes_flat_lines() {
        let buf = Shared::default();
        let mut log = EventLog::to_writer(buf.clone());
        log.write(Timestamp(42), &LogEvent::RenderBegin { frame_id: 3 });
        log.write(Timestamp(43), &LogEvent::Close);
        let text = String::from_utf8(buf.0.borrow().clone()).unwrap();
        assert_eq!(
            text,
            "{\"t\":42,\"event\":\"render_begin\",\"frame_id\":3}\n{\"t\":43,\"event\":\"close\"}\n"
        );
    }

    #[test]
    fn disabled_log_writes_nothing() {
        let mut log = EventLog::disabled();
        log.write(Timestamp(1), &LogEvent::Close);
    }
}
