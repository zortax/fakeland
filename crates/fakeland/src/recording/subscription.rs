//! Live record streams. Each subscriber has a bounded queue. A full queue drops records and the
//! next delivered item is a gap marker with the count of dropped records, so a slow subscriber
//! never blocks the compositor.

use std::pin::Pin;
use std::task::{Context, Poll};

use fakeland_trace::Record;
use futures_channel::mpsc;
use futures_util::{Stream, StreamExt};

use crate::util::block_on;

/// One item of a subscription.
#[derive(Debug, Clone, PartialEq)]
pub enum SubscriptionItem {
    /// A trace record.
    Record(Record),
    /// The subscriber lagged and lost this many records here.
    Gap {
        /// The count of lost records.
        missed: u64,
    },
}

/// A stream of live records. It ends when the session shuts down.
#[derive(Debug)]
pub struct Subscription {
    rx: mpsc::Receiver<SubscriptionItem>,
}

impl Subscription {
    /// Waits for the next item on the calling thread. `None` means the session ended.
    pub fn blocking_next(&mut self) -> Option<SubscriptionItem> {
        block_on(self.rx.next())
    }

    /// Returns the next queued item without waiting. `Ok(None)` means the session ended, and
    /// `Err(())` means no item is queued.
    #[allow(clippy::result_unit_err)]
    pub fn try_next(&mut self) -> Result<Option<SubscriptionItem>, ()> {
        match self.rx.try_recv() {
            Ok(item) => Ok(Some(item)),
            Err(mpsc::TryRecvError::Closed) => Ok(None),
            Err(_) => Err(()),
        }
    }
}

impl Stream for Subscription {
    type Item = SubscriptionItem;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx).poll_next(cx)
    }
}

/// The compositor side of one subscription.
#[derive(Debug)]
pub struct Subscriber {
    tx: mpsc::Sender<SubscriptionItem>,
    missed: u64,
}

impl Subscriber {
    /// Builds a subscriber and its stream with room for `capacity` items.
    pub fn pair(capacity: usize) -> (Self, Subscription) {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        (Self { tx, missed: 0 }, Subscription { rx })
    }

    /// Offers a record. Returns false when the stream is gone.
    pub fn offer(&mut self, record: &Record) -> bool {
        if self.missed > 0 {
            match self.tx.try_send(SubscriptionItem::Gap {
                missed: self.missed,
            }) {
                Ok(()) => self.missed = 0,
                Err(e) if e.is_disconnected() => return false,
                Err(_) => {
                    self.missed += 1;
                    return true;
                }
            }
        }
        match self.tx.try_send(SubscriptionItem::Record(record.clone())) {
            Ok(()) => true,
            Err(e) if e.is_disconnected() => false,
            Err(_) => {
                self.missed += 1;
                true
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use fakeland_trace::Event;
    use fakeland_types::Timestamp;

    use super::*;

    fn mark(i: u64) -> Record {
        Record::new(
            Timestamp(i),
            Event::UserMark {
                label: format!("m{i}"),
            },
        )
    }

    #[test]
    fn a_slow_subscriber_gets_a_gap_marker() {
        let (mut sub, mut stream) = Subscriber::pair(1);
        for i in 0..10 {
            assert!(sub.offer(&mark(i)));
        }
        let mut got = Vec::new();
        while let Ok(Some(item)) = stream.try_next() {
            got.push(item);
        }
        assert!(matches!(got[0], SubscriptionItem::Record(_)));
        assert!(got.len() < 10);
        assert!(sub.offer(&mark(10)));
        let next = stream.try_next().unwrap().unwrap();
        let SubscriptionItem::Gap { missed } = next else {
            panic!("expected a gap, got {next:?}");
        };
        assert_eq!(missed as usize + got.len(), 10);
        assert_eq!(
            stream.try_next().unwrap(),
            Some(SubscriptionItem::Record(mark(10)))
        );
    }

    #[test]
    fn a_dropped_stream_ends_the_subscriber() {
        let (mut sub, stream) = Subscriber::pair(4);
        drop(stream);
        assert!(!sub.offer(&mark(0)));
    }
}
