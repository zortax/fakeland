//! The record buffer. One thread owns it; a push is O(1) and takes no lock.

use std::collections::{BTreeMap, VecDeque};

use fakeland_types::{Clock, Timestamp};

use crate::context::ContextLog;
use crate::event::{Event, Record};
use crate::recorder::outputs::outputs_at;
use crate::recorder::{CaptureId, RecorderError};
use crate::trace::{Trace, TraceHeader};

/// The retention policy of a recorder.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RecorderMode {
    /// Keep every record.
    #[default]
    Unbounded,
    /// Keep the last `capacity` records. Older records drop out.
    Ring {
        /// The number of records to keep. Zero counts as one.
        capacity: usize,
    },
}

/// The minimum context size that starts a compaction.
const COMPACT_MIN: usize = 1024;

/// An append-only buffer of trace records.
///
/// Every record has an absolute index that counts from zero over the life of the recorder. In ring
/// mode the oldest records drop out, and their indices stay below [`Recorder::first_index`]. The
/// recorder keeps the context records (outputs, clients, processes, surfaces, windows) of live
/// entities apart, so every trace it returns starts with the context that its records need.
#[derive(Debug)]
pub struct Recorder {
    header: TraceHeader,
    capacity: Option<usize>,
    records: VecDeque<Record>,
    base: u64,
    context: ContextLog,
    captures: BTreeMap<CaptureId, u64>,
    next_capture: u64,
    compact_at: usize,
}

impl Recorder {
    /// Builds an empty recorder.
    pub fn new(header: TraceHeader, mode: RecorderMode) -> Self {
        let capacity = match mode {
            RecorderMode::Unbounded => None,
            RecorderMode::Ring { capacity } => Some(capacity.max(1)),
        };
        Self {
            header,
            capacity,
            records: capacity.map_or_else(VecDeque::new, VecDeque::with_capacity),
            base: 0,
            context: ContextLog::new(),
            captures: BTreeMap::new(),
            next_capture: 0,
            compact_at: COMPACT_MIN,
        }
    }

    /// Appends an event at a time.
    pub fn push(&mut self, t: Timestamp, event: Event) {
        self.push_record(Record::new(t, event));
    }

    /// Appends an event at the current time of a clock.
    pub fn push_now(&mut self, clock: &dyn Clock, event: Event) {
        self.push(clock.now(), event);
    }

    /// Appends a record.
    pub fn push_record(&mut self, record: Record) {
        let index = self.end_index();
        if record.event.context_key().is_some() {
            self.context.observe(index, &record);
        }
        if let Some(cap) = self.capacity {
            if self.records.len() >= cap {
                self.records.pop_front();
                self.base += 1;
            }
            if self.context.len() >= self.compact_at {
                self.context.compact(self.horizon());
                self.compact_at = (self.context.len() * 2).max(COMPACT_MIN);
            }
        }
        self.records.push_back(record);
    }

    /// Returns the header.
    pub const fn header(&self) -> &TraceHeader {
        &self.header
    }

    /// Returns the header for changes, for example to fill in the environment.
    pub const fn header_mut(&mut self) -> &mut TraceHeader {
        &mut self.header
    }

    /// Returns the retention policy.
    pub fn mode(&self) -> RecorderMode {
        self.capacity
            .map_or(RecorderMode::Unbounded, |capacity| RecorderMode::Ring {
                capacity,
            })
    }

    /// Returns the number of held records.
    pub fn len(&self) -> usize {
        self.records.len()
    }

    /// Returns true when the recorder holds no records.
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    /// Returns the absolute index of the oldest held record. It equals the number of records that
    /// dropped out or were taken.
    pub const fn first_index(&self) -> u64 {
        self.base
    }

    /// Returns the absolute index that the next record gets.
    pub fn end_index(&self) -> u64 {
        self.base + self.records.len() as u64
    }

    /// Returns the held records in order.
    pub fn iter(&self) -> impl Iterator<Item = &Record> + '_ {
        self.records.iter()
    }

    /// Returns the held records from absolute index `cursor` on. A stream reader keeps
    /// [`Recorder::end_index`] as its next cursor. A cursor below [`Recorder::first_index`] has
    /// lost records.
    pub fn since(&self, cursor: u64) -> impl Iterator<Item = &Record> + '_ {
        let skip = cursor
            .saturating_sub(self.base)
            .min(self.records.len() as u64) as usize;
        self.records.range(skip..)
    }

    /// Opens a capture span at the next record.
    pub fn begin_capture(&mut self) -> CaptureId {
        let id = CaptureId(self.next_capture);
        self.next_capture += 1;
        self.captures.insert(id, self.end_index());
        id
    }

    /// Returns the open capture spans.
    pub fn open_captures(&self) -> impl Iterator<Item = CaptureId> + '_ {
        self.captures.keys().copied()
    }

    /// Closes a capture span and returns its trace: the live context, then every record since
    /// [`Recorder::begin_capture`]. The header counts records that dropped out of the ring.
    pub fn end_capture(&mut self, id: CaptureId) -> Result<Trace, RecorderError> {
        let start = self
            .captures
            .remove(&id)
            .ok_or(RecorderError::UnknownCapture(id))?;
        Ok(self.trace_from(start))
    }

    /// Returns a copy of every held record as a trace, preceded by the live context.
    pub fn snapshot(&self) -> Trace {
        let mut trace = self.trace_from(self.base);
        trace.header.dropped = self.base;
        trace
    }

    /// Returns every held record as a trace and empties the buffer. Open captures lose the taken
    /// records.
    pub fn take(&mut self) -> Trace {
        let trace = self.snapshot();
        self.base = self.end_index();
        self.records.clear();
        trace
    }

    fn trace_from(&self, start: u64) -> Trace {
        let first = start.max(self.base);
        let mut records = self.context.live_before(first);
        let mut header = self.header.clone();
        header.outputs = outputs_at(&self.header.outputs, &records);
        header.dropped = first - start;
        if first > 0 {
            let begin = self
                .since(first)
                .next()
                .or(self.records.back())
                .map(|r| r.t);
            if let Some(begin) = begin {
                header.started = header.started.max(begin);
            }
        }
        records.extend(self.since(first).cloned());
        Trace { header, records }
    }

    fn horizon(&self) -> u64 {
        self.captures
            .values()
            .copied()
            .min()
            .unwrap_or(u64::MAX)
            .min(self.base)
    }
}
