//! Trace calls of the session: capture spans, snapshots, live subscriptions and run validity.

use fakeland_trace::{CaptureId, Trace};

use super::{Session, SessionError};
use crate::display::RunValidity;
use crate::recording::Subscription;
use crate::util::block_on;

impl Session {
    /// Starts a trace capture span.
    pub async fn begin_capture(&self) -> Result<CaptureId, SessionError> {
        self.inner.call(|state| state.bus.begin_capture()).await
    }

    /// Starts a trace capture span on the calling thread.
    pub fn blocking_begin_capture(&self) -> Result<CaptureId, SessionError> {
        block_on(self.begin_capture())
    }

    /// Ends a capture span and returns its trace: the live context, then every record since the
    /// span began.
    pub async fn end_capture(&self, id: CaptureId) -> Result<Trace, SessionError> {
        Ok(self
            .inner
            .call(move |state| state.bus.end_capture(id))
            .await??)
    }

    /// Ends a capture span on the calling thread.
    pub fn blocking_end_capture(&self, id: CaptureId) -> Result<Trace, SessionError> {
        block_on(self.end_capture(id))
    }

    /// Returns a copy of every record the ring holds.
    pub async fn trace_snapshot(&self) -> Result<Trace, SessionError> {
        self.inner
            .call(|state| state.bus.recorder().snapshot())
            .await
    }

    /// Returns a trace snapshot on the calling thread.
    pub fn blocking_trace_snapshot(&self) -> Result<Trace, SessionError> {
        block_on(self.trace_snapshot())
    }

    /// Subscribes to live records. A subscriber that lags loses records and gets a gap marker.
    pub async fn subscribe(&self, capacity: usize) -> Result<Subscription, SessionError> {
        self.inner
            .call(move |state| state.bus.subscribe(capacity))
            .await
    }

    /// Subscribes on the calling thread.
    pub fn blocking_subscribe(&self, capacity: usize) -> Result<Subscription, SessionError> {
        block_on(self.subscribe(capacity))
    }

    /// Returns the validity of the run from the wake lateness so far.
    pub async fn validity(&self) -> Result<RunValidity, SessionError> {
        self.inner.call(|state| state.lateness.validity()).await
    }

    /// Returns the run validity on the calling thread.
    pub fn blocking_validity(&self) -> Result<RunValidity, SessionError> {
        block_on(self.validity())
    }

    /// Clears the lateness counts, for example after a warmup.
    pub async fn reset_validity(&self) -> Result<(), SessionError> {
        self.inner.call(|state| state.lateness.reset()).await
    }
}
