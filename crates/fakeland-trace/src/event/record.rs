//! A trace record: one event at one compositor timestamp.

use fakeland_types::Timestamp;
use serde::{Deserialize, Serialize};

use crate::event::Event;

/// One event at the `CLOCK_MONOTONIC` time the compositor thread recorded it.
///
/// The record serializes flat: `{"t": 123, "type": "commit", ...}`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// The time of the event.
    pub t: Timestamp,
    /// The event.
    #[serde(flatten)]
    pub event: Event,
}

impl Record {
    /// Builds a record.
    pub const fn new(t: Timestamp, event: Event) -> Self {
        Self { t, event }
    }
}
