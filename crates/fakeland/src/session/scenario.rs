//! Scenario calls of the session: start a run and get a handle that resolves to its result.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use std::task::{Context, Poll};

use fakeland_scenario::Scenario;
use fakeland_types::RunId;
use futures_channel::oneshot;
use futures_util::FutureExt;

use super::{Session, SessionError, SessionInner};
use crate::runner::{RunOptions, RunResult};
use crate::util::block_on;

/// A running scenario. Awaiting the handle returns the result once the run ends.
#[derive(Debug)]
pub struct RunHandle {
    run: RunId,
    inner: Weak<SessionInner>,
    rx: oneshot::Receiver<RunResult>,
}

impl RunHandle {
    /// Returns the run id.
    pub fn id(&self) -> RunId {
        self.run
    }

    /// Cancels the run. Returns false when the run had ended.
    pub async fn cancel(&self) -> Result<bool, SessionError> {
        let inner = self.inner.upgrade().ok_or(SessionError::Closed)?;
        let run = self.run;
        inner.call(move |state| state.cancel_run(run)).await
    }

    /// Cancels the run on the calling thread.
    pub fn blocking_cancel(&self) -> Result<bool, SessionError> {
        block_on(self.cancel())
    }

    /// Returns the result when the run has ended.
    pub fn try_result(&mut self) -> Option<RunResult> {
        self.rx.try_recv().ok().flatten()
    }

    /// Waits for the result on the calling thread.
    pub fn blocking_wait(self) -> Result<RunResult, SessionError> {
        block_on(self)
    }
}

impl Future for RunHandle {
    type Output = Result<RunResult, SessionError>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.rx
            .poll_unpin(cx)
            .map(|r| r.map_err(|_| SessionError::Closed))
    }
}

impl Session {
    /// Starts a scenario run on the compositor thread.
    pub async fn run(&self, scenario: Scenario) -> Result<RunHandle, SessionError> {
        self.run_with(scenario, RunOptions::default()).await
    }

    /// Starts a scenario run on the calling thread.
    pub fn blocking_run(&self, scenario: Scenario) -> Result<RunHandle, SessionError> {
        block_on(self.run(scenario))
    }

    /// Starts a scenario run with frame captures.
    pub async fn run_with(
        &self,
        scenario: Scenario,
        options: RunOptions,
    ) -> Result<RunHandle, SessionError> {
        let (tx, rx) = oneshot::channel();
        let run = self
            .inner
            .call(move |state| state.start_run(scenario, options, tx))
            .await?;
        Ok(RunHandle {
            run,
            inner: Arc::downgrade(&self.inner),
            rx,
        })
    }

    /// Starts a scenario run with frame captures on the calling thread.
    pub fn blocking_run_with(
        &self,
        scenario: Scenario,
        options: RunOptions,
    ) -> Result<RunHandle, SessionError> {
        block_on(self.run_with(scenario, options))
    }

    /// Cancels a run by id. Returns false when the run had ended.
    pub async fn cancel(&self, run: RunId) -> Result<bool, SessionError> {
        self.inner.call(move |state| state.cancel_run(run)).await
    }

    /// Cancels a run by id on the calling thread.
    pub fn blocking_cancel(&self, run: RunId) -> Result<bool, SessionError> {
        block_on(self.cancel(run))
    }
}
