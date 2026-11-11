//! A subscription on the client side: a stream of records, gaps and the final close.

use std::pin::Pin;
use std::sync::Weak;
use std::task::{Context, Poll};

use fakeland_trace::Record;
use futures_channel::mpsc::UnboundedReceiver;
use futures_util::{Stream, StreamExt};

use super::connection::Inner;

/// One subscription event.
#[derive(Clone, Debug, PartialEq)]
pub enum SubEvent {
    /// A trace record with its compositor timestamp.
    Record(Record),
    /// The server dropped this many records because the client read too slowly.
    Gap {
        /// The count of dropped records.
        missed: u64,
    },
}

/// A stream of subscription events. It ends when the server closes the subscription or the
/// connection. Dropping it unsubscribes.
#[derive(Debug)]
pub struct ClientSubscription {
    id: u64,
    rx: UnboundedReceiver<SubEvent>,
    client: Weak<Inner>,
}

impl ClientSubscription {
    pub(super) fn new(id: u64, rx: UnboundedReceiver<SubEvent>, client: Weak<Inner>) -> Self {
        Self { id, rx, client }
    }

    /// Returns the subscription id.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Returns the next event on the calling thread, or `None` at the end.
    pub fn blocking_next(&mut self) -> Option<SubEvent> {
        fakeland::block_on(self.rx.next())
    }
}

impl Stream for ClientSubscription {
    type Item = SubEvent;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<SubEvent>> {
        self.rx.poll_next_unpin(cx)
    }
}

impl Drop for ClientSubscription {
    fn drop(&mut self) {
        if let Some(client) = self.client.upgrade() {
            client.forget_subscription(self.id);
        }
    }
}
