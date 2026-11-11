//! The event bus of the compositor thread. Every trace event goes into the recorder, to each live
//! subscriber and into the queue of in-thread observers.

use fakeland_trace::recorder::RecorderError;
use fakeland_trace::{CaptureId, Event, Record, Recorder, RecorderMode, Trace, TraceHeader};
use fakeland_types::Timestamp;

use super::{Subscriber, Subscription};

/// Fans out trace events.
#[derive(Debug)]
pub struct EventBus {
    recorder: Recorder,
    subscribers: Vec<Subscriber>,
    observed: Vec<Record>,
    observe: bool,
    last: Timestamp,
}

impl EventBus {
    /// Builds a bus over a recorder.
    pub fn new(header: TraceHeader, capacity: Option<usize>) -> Self {
        let mode = match capacity {
            Some(capacity) => RecorderMode::Ring { capacity },
            None => RecorderMode::Unbounded,
        };
        let last = header.started;
        Self {
            recorder: Recorder::new(header, mode),
            subscribers: Vec::new(),
            observed: Vec::new(),
            observe: false,
            last,
        }
    }

    /// Records an event at `t`. A time before the previous record moves up to it, so record
    /// times never decrease.
    pub fn push(&mut self, t: Timestamp, event: Event) -> Timestamp {
        let t = t.max(self.last);
        self.last = t;
        let record = Record::new(t, event);
        if !self.subscribers.is_empty() {
            self.subscribers.retain_mut(|s| s.offer(&record));
        }
        if self.observe {
            self.observed.push(record.clone());
        }
        self.recorder.push_record(record);
        t
    }

    /// Turns the observer queue on or off.
    pub fn set_observing(&mut self, on: bool) {
        self.observe = on;
        if !on {
            self.observed.clear();
        }
    }

    /// Takes the records queued for observers.
    pub fn take_observed(&mut self) -> Vec<Record> {
        std::mem::take(&mut self.observed)
    }

    /// Adds a live subscriber with room for `capacity` items.
    pub fn subscribe(&mut self, capacity: usize) -> Subscription {
        let (sub, stream) = Subscriber::pair(capacity);
        self.subscribers.push(sub);
        stream
    }

    /// Drops every subscriber, which ends their streams.
    pub fn close_subscribers(&mut self) {
        self.subscribers.clear();
    }

    /// Returns the recorder.
    pub fn recorder(&self) -> &Recorder {
        &self.recorder
    }

    /// Returns the recorder for changes to its header.
    pub fn recorder_mut(&mut self) -> &mut Recorder {
        &mut self.recorder
    }

    /// Starts a capture span.
    pub fn begin_capture(&mut self) -> CaptureId {
        self.recorder.begin_capture()
    }

    /// Ends a capture span and returns its trace.
    pub fn end_capture(&mut self, id: CaptureId) -> Result<Trace, RecorderError> {
        self.recorder.end_capture(id)
    }
}

#[cfg(test)]
mod tests {
    use fakeland_trace::Environment;

    use super::*;

    #[test]
    fn times_never_decrease_and_observers_see_records() {
        let header = TraceHeader::new(Environment::default(), Vec::new(), Timestamp(10));
        let mut bus = EventBus::new(header, Some(16));
        bus.set_observing(true);
        let mark = |l: &str| Event::UserMark { label: l.into() };
        assert_eq!(bus.push(Timestamp(20), mark("a")), Timestamp(20));
        assert_eq!(bus.push(Timestamp(15), mark("b")), Timestamp(20));
        let observed = bus.take_observed();
        assert_eq!(observed.len(), 2);
        assert!(bus.take_observed().is_empty());
        let id = bus.begin_capture();
        bus.push(Timestamp(30), mark("c"));
        let trace = bus.end_capture(id).unwrap();
        assert!(
            trace
                .records
                .iter()
                .any(|r| matches!(&r.event, Event::UserMark { label } if label == "c"))
        );
    }
}
