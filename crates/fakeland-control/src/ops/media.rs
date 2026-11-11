//! Resource and recording operations.

use fakeland::{RecordSpec, RecordingInfo, RecordingReport, ResourceSnapshot};

use super::{Controller, OpError};
use crate::methods::{NoParams, RecordStopParams};

impl Controller {
    /// Returns the latest resource samples.
    pub async fn resources(&self, _: NoParams) -> Result<ResourceSnapshot, OpError> {
        Ok(self.session().resources().await?)
    }

    /// Starts a recording. The path must be absolute, since the server writes it.
    pub async fn record_start(&self, spec: RecordSpec) -> Result<RecordingInfo, OpError> {
        if !spec.path.is_absolute() {
            return Err(OpError::InvalidParams(format!(
                "the recording path must be absolute: {}",
                spec.path.display()
            )));
        }
        let recording = self.session().record(spec).await?;
        self.session()
            .recordings()
            .await?
            .into_iter()
            .find(|r| r.id == recording.id())
            .ok_or_else(|| OpError::Failed("the recording ended at once".into()))
    }

    /// Stops a recording, or the newest one.
    pub async fn record_stop(&self, p: RecordStopParams) -> Result<RecordingReport, OpError> {
        let id = match p.id {
            Some(id) => id,
            None => self
                .session()
                .recordings()
                .await?
                .iter()
                .map(|r| r.id)
                .max()
                .ok_or_else(|| OpError::NotFound("no recording runs".into()))?,
        };
        Ok(self.session().stop_recording(id).await?)
    }

    /// Returns every running recording.
    pub async fn recordings(&self, _: NoParams) -> Result<Vec<RecordingInfo>, OpError> {
        Ok(self.session().recordings().await?)
    }
}
