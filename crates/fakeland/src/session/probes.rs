//! Probe, wait and frame capture calls of the session.

use fakeland_scenario::{ProbeSpec, Wait};
use fakeland_trace::InputSource;

use super::{Session, SessionError};
use crate::actions::ActionCtx;
use crate::framecap::{CaptureSpec, FrameCapture, FrameCaptureId};
use crate::probes::ProbeReport;
use crate::util::{CancelToken, block_on};
use crate::waits::WaitOutput;

impl Session {
    /// Runs a latency probe and returns the report of every repeat.
    pub async fn probe(&self, spec: ProbeSpec) -> Result<ProbeReport, SessionError> {
        self.inner
            .call_with(move |state, reply| {
                state.start_probe(
                    &spec,
                    ActionCtx::new(InputSource::Api),
                    Box::new(move |_, report| {
                        let _ = reply.send(report);
                    }),
                );
            })
            .await
    }

    /// Runs a latency probe on the calling thread.
    pub fn blocking_probe(&self, spec: ProbeSpec) -> Result<ProbeReport, SessionError> {
        block_on(self.probe(spec))
    }

    /// Waits on the compositor thread for a scenario wait condition.
    pub async fn wait(&self, wait: Wait) -> Result<WaitOutput, SessionError> {
        self.inner
            .call_with(move |state, reply| {
                state.start_wait(
                    &wait,
                    CancelToken::new(),
                    Box::new(move |_, result| {
                        let _ = reply.send(result.map_err(SessionError::from));
                    }),
                );
            })
            .await?
    }

    /// Waits for a condition on the calling thread.
    pub fn blocking_wait(&self, wait: Wait) -> Result<WaitOutput, SessionError> {
        block_on(self.wait(wait))
    }

    /// Starts a frame capture.
    pub async fn begin_frame_capture(
        &self,
        spec: CaptureSpec,
    ) -> Result<FrameCaptureId, SessionError> {
        Ok(self
            .inner
            .call(move |state| state.begin_frame_capture(spec, None))
            .await??)
    }

    /// Starts a frame capture on the calling thread.
    pub fn blocking_begin_frame_capture(
        &self,
        spec: CaptureSpec,
    ) -> Result<FrameCaptureId, SessionError> {
        block_on(self.begin_frame_capture(spec))
    }

    /// Ends a frame capture and returns its frames.
    pub async fn end_frame_capture(
        &self,
        id: FrameCaptureId,
    ) -> Result<FrameCapture, SessionError> {
        self.inner
            .call(move |state| state.end_frame_capture(id))
            .await?
            .ok_or(SessionError::NoFrameCapture)
    }

    /// Ends a frame capture on the calling thread.
    pub fn blocking_end_frame_capture(
        &self,
        id: FrameCaptureId,
    ) -> Result<FrameCapture, SessionError> {
        block_on(self.end_frame_capture(id))
    }
}
