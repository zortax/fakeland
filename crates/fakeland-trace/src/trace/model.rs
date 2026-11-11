//! The trace model and its time slicing.

use std::ops::{Bound, RangeBounds};

use fakeland_types::{Nanos, Timestamp};
use serde::{Deserialize, Serialize};

use crate::context::ContextLog;
use crate::event::{Event, Record};
use crate::trace::TraceHeader;

/// A header and records in time order.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    /// The header.
    pub header: TraceHeader,
    /// The records in time order.
    pub records: Vec<Record>,
}

impl Trace {
    /// Builds an empty trace.
    pub const fn new(header: TraceHeader) -> Self {
        Self {
            header,
            records: Vec::new(),
        }
    }

    /// Appends a record.
    pub fn push(&mut self, t: Timestamp, event: Event) {
        self.records.push(Record::new(t, event));
    }

    /// Returns the time of the first record, or the header start for an empty trace.
    pub fn start(&self) -> Timestamp {
        self.records.first().map_or(self.header.started, |r| r.t)
    }

    /// Returns the time of the last record, or the header start for an empty trace.
    pub fn end(&self) -> Timestamp {
        self.records.last().map_or(self.header.started, |r| r.t)
    }

    /// Returns the span from the header start to the last record.
    pub fn duration(&self) -> Nanos {
        let start = self.header.started.min(self.start());
        self.end() - start
    }

    /// Returns the records whose time lies in `range`, preceded by the context records that are
    /// live at the start of the range. Context records are output, client, process, surface and
    /// window events, so the slice stays readable on its own. The header start moves to the range
    /// start.
    pub fn slice(&self, range: impl RangeBounds<Timestamp>) -> Trace {
        let first = self.records.iter().position(|r| range.contains(&r.t));
        let mut header = self.header.clone();
        if let Bound::Included(s) | Bound::Excluded(s) = range.start_bound() {
            header.started = header.started.max(*s);
        }
        let Some(first) = first else {
            return Trace::new(header);
        };
        let mut log = ContextLog::new();
        for (i, r) in self.records[..first].iter().enumerate() {
            log.observe(i as u64, r);
        }
        let mut records = log.live_before(first as u64);
        records.extend(
            self.records[first..]
                .iter()
                .filter(|r| range.contains(&r.t))
                .cloned(),
        );
        Trace { header, records }
    }

    /// Returns a trace with the records that satisfy a predicate.
    pub fn filter(&self, keep: impl Fn(&Record) -> bool) -> Trace {
        Trace {
            header: self.header.clone(),
            records: self.records.iter().filter(|r| keep(r)).cloned().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::{ClientId, SurfaceId};

    use super::*;
    use crate::event::Environment;

    #[test]
    fn slice_keeps_live_context() {
        let mut t = Trace::new(TraceHeader::new(
            Environment::default(),
            vec![],
            Timestamp(0),
        ));
        t.push(
            Timestamp(1),
            Event::SurfaceCreated {
                surface: SurfaceId(1),
                client: ClientId(1),
            },
        );
        t.push(Timestamp(2), Event::UserMark { label: "a".into() });
        t.push(Timestamp(3), Event::UserMark { label: "b".into() });
        t.push(Timestamp(4), Event::UserMark { label: "c".into() });
        let s = t.slice(Timestamp(3)..Timestamp(4));
        assert_eq!(s.records.len(), 2);
        assert_eq!(s.records[0].event.name(), "surface_created");
        assert_eq!(s.records[1].t, Timestamp(3));
        assert_eq!(s.header.started, Timestamp(3));
        assert!(t.slice(Timestamp(10)..).records.is_empty());
        assert_eq!(t.slice(..).records.len(), 4);
    }
}
