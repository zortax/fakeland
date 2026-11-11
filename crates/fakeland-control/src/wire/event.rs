//! Events: messages the server sends without a request. Subscriptions stream trace records, and
//! finished runs report their summary.

use fakeland_trace::Record;
use serde::{Deserialize, Serialize};

use crate::methods::RunSummary;

/// A message without a request id.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Event {
    /// What happened.
    pub event: EventBody,
}

/// The content of an event.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventBody {
    /// A trace record that matches a subscription. The record carries its compositor timestamp.
    Record {
        /// The subscription id.
        subscription: u64,
        /// The record.
        record: Record,
    },
    /// The subscription queue overflowed and dropped records.
    Gap {
        /// The subscription id.
        subscription: u64,
        /// The count of dropped records.
        missed: u64,
    },
    /// The subscription ended because the session closed or the client unsubscribed.
    Closed {
        /// The subscription id.
        subscription: u64,
    },
    /// A scenario run that this connection started has finished.
    RunFinished {
        /// The run summary.
        summary: Box<RunSummary>,
    },
}

impl Event {
    /// Wraps an event body.
    pub fn new(event: EventBody) -> Self {
        Self { event }
    }

    /// Returns the subscription the event belongs to.
    pub fn subscription(&self) -> Option<u64> {
        match &self.event {
            EventBody::Record { subscription, .. }
            | EventBody::Gap { subscription, .. }
            | EventBody::Closed { subscription } => Some(*subscription),
            EventBody::RunFinished { .. } => None,
        }
    }
}
