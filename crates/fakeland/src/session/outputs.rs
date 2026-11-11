//! Output calls of the session: listing, runtime changes and hotplug.

use fakeland_types::OutputId;

use super::{Session, SessionError};
use crate::config::OutputConfig;
use crate::outputs::{OutputChange, OutputState};
use crate::util::block_on;

impl Session {
    /// Returns every output in order.
    pub async fn outputs(&self) -> Result<Vec<OutputState>, SessionError> {
        self.inner
            .call(|state| state.outputs.iter().map(|o| o.state()).collect())
            .await
    }

    /// Returns every output on the calling thread.
    pub fn blocking_outputs(&self) -> Result<Vec<OutputState>, SessionError> {
        block_on(self.outputs())
    }

    /// Applies a runtime change to an output. `None` selects the primary output.
    pub async fn set_output(
        &self,
        output: Option<OutputId>,
        change: OutputChange,
    ) -> Result<(), SessionError> {
        self.inner
            .call(move |state| {
                let id =
                    state
                        .outputs
                        .resolve(output)
                        .ok_or(crate::outputs::OutputError::NoOutput(
                            output.unwrap_or(OutputId(0)),
                        ))?;
                state.change_output(id, change)
            })
            .await??;
        Ok(())
    }

    /// Applies a runtime change on the calling thread.
    pub fn blocking_set_output(
        &self,
        output: Option<OutputId>,
        change: OutputChange,
    ) -> Result<(), SessionError> {
        block_on(self.set_output(output, change))
    }

    /// Plugs in a new output.
    pub async fn add_output(&self, config: OutputConfig) -> Result<OutputId, SessionError> {
        Ok(self
            .inner
            .call(move |state| state.add_output(config))
            .await??)
    }

    /// Plugs in a new output on the calling thread.
    pub fn blocking_add_output(&self, config: OutputConfig) -> Result<OutputId, SessionError> {
        block_on(self.add_output(config))
    }

    /// Unplugs an output. Its windows move to the primary output.
    pub async fn remove_output(&self, output: OutputId) -> Result<(), SessionError> {
        Ok(self
            .inner
            .call(move |state| state.remove_output(output))
            .await??)
    }

    /// Unplugs an output on the calling thread.
    pub fn blocking_remove_output(&self, output: OutputId) -> Result<(), SessionError> {
        block_on(self.remove_output(output))
    }
}
