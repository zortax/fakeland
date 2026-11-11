//! The caller side of a recording: start through the session, the `Recording` handle and stop.
//! Opening the encoder and waiting for it at the end run off the compositor thread.

use std::sync::Weak;

use futures_channel::oneshot;

use super::service::Stopped;
use super::writer::open_writer;
use super::{RecordError, RecordSpec, RecordingId, RecordingInfo, RecordingReport};
use crate::session::{Session, SessionError, SessionInner};
use crate::util::block_on;

/// A running recording. Stop it with [`Recording::stop`] or [`Session::stop_recording`]; a
/// recording without a stop ends at session shutdown.
#[derive(Debug, Clone)]
pub struct Recording {
    id: RecordingId,
    session: Weak<SessionInner>,
}

impl Recording {
    /// Returns the id.
    pub fn id(&self) -> RecordingId {
        self.id
    }

    /// Stops the recording and waits until the file is complete.
    pub async fn stop(&self) -> Result<RecordingReport, SessionError> {
        let session = self
            .session
            .upgrade()
            .map(Session::from_inner)
            .ok_or(SessionError::Closed)?;
        session.stop_recording(self.id).await
    }

    /// Stops the recording on the calling thread.
    pub fn blocking_stop(&self) -> Result<RecordingReport, SessionError> {
        block_on(self.stop())
    }
}

/// Waits for the writer on a helper thread, so an async caller never blocks.
async fn finish(stopped: Stopped) -> Result<RecordingReport, SessionError> {
    let (tx, rx) = oneshot::channel();
    std::thread::Builder::new()
        .name("fakeland-video-end".into())
        .spawn(move || {
            let _ = tx.send(stopped.finish());
        })
        .map_err(|e| {
            SessionError::Record(RecordError::Io {
                path: Default::default(),
                message: e.to_string(),
            })
        })?;
    rx.await.map_err(|_| SessionError::Closed)
}

impl Session {
    /// Starts a recording of an output or a window. Frames follow the vblanks of the output, one
    /// slot per refresh unless the spec sets a rate.
    pub async fn record(&self, spec: RecordSpec) -> Result<Recording, SessionError> {
        let plan = self
            .inner
            .call(move |state| state.plan_recording(spec))
            .await??;
        let (queue, writer) = open_writer(plan.format, &plan.spec.path, plan.canvas, plan.rate)?;
        let armed = self
            .inner
            .call(move |state| state.arm_recording(plan, queue, writer))
            .await?;
        match armed {
            Ok(id) => Ok(Recording {
                id,
                session: std::sync::Arc::downgrade(&self.inner),
            }),
            Err(failed) => {
                let (e, stopped) = *failed;
                let _ = finish(stopped).await;
                Err(e.into())
            }
        }
    }

    /// Starts a recording on the calling thread.
    pub fn blocking_record(&self, spec: RecordSpec) -> Result<Recording, SessionError> {
        block_on(self.record(spec))
    }

    /// Stops a recording and waits until the file is complete.
    pub async fn stop_recording(&self, id: RecordingId) -> Result<RecordingReport, SessionError> {
        let stopped = self
            .inner
            .call(move |state| state.stop_recording(id))
            .await?
            .ok_or(SessionError::Record(RecordError::NotFound))?;
        finish(stopped).await
    }

    /// Stops a recording on the calling thread.
    pub fn blocking_stop_recording(
        &self,
        id: RecordingId,
    ) -> Result<RecordingReport, SessionError> {
        block_on(self.stop_recording(id))
    }

    /// Returns every running recording.
    pub async fn recordings(&self) -> Result<Vec<RecordingInfo>, SessionError> {
        self.inner.call(|state| state.recording_infos()).await
    }

    /// Returns every running recording on the calling thread.
    pub fn blocking_recordings(&self) -> Result<Vec<RecordingInfo>, SessionError> {
        block_on(self.recordings())
    }
}
