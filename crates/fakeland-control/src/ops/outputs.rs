//! Output operations.

use fakeland::OutputState;
use fakeland_types::OutputId;

use super::{Controller, OpError};
use crate::methods::{AddOutputParams, NoParams, RemoveOutputParams, SetOutputParams};

impl Controller {
    /// Returns every output.
    pub async fn outputs(&self, _: NoParams) -> Result<Vec<OutputState>, OpError> {
        Ok(self.session().outputs().await?)
    }

    async fn output_state(&self, id: Option<OutputId>) -> Result<OutputState, OpError> {
        let outputs = self.session().outputs().await?;
        let found = match id {
            Some(id) => outputs.into_iter().find(|o| o.id == id),
            None => outputs.into_iter().next(),
        };
        found.ok_or_else(|| OpError::NotFound("no such output".into()))
    }

    /// Changes an output and returns its new state.
    pub async fn set_output(&self, p: SetOutputParams) -> Result<OutputState, OpError> {
        self.session().set_output(p.output, p.change).await?;
        self.output_state(p.output).await
    }

    /// Adds an output.
    pub async fn add_output(&self, p: AddOutputParams) -> Result<OutputState, OpError> {
        let id = self.session().add_output(p.config).await?;
        self.output_state(Some(id)).await
    }

    /// Removes an output.
    pub async fn remove_output(&self, p: RemoveOutputParams) -> Result<(), OpError> {
        Ok(self.session().remove_output(p.output).await?)
    }
}
