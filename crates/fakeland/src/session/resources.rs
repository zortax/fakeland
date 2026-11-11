//! Resource calls of the session: the latest process tree and GPU samples.

use super::{Session, SessionError};
use crate::sampling::ResourceSnapshot;
use crate::util::block_on;

impl Session {
    /// Returns the latest resource samples. A session without sampling returns a disabled
    /// snapshot.
    pub async fn resources(&self) -> Result<ResourceSnapshot, SessionError> {
        self.inner.call(|state| state.resource_snapshot()).await
    }

    /// Returns the latest resource samples on the calling thread.
    pub fn blocking_resources(&self) -> Result<ResourceSnapshot, SessionError> {
        block_on(self.resources())
    }
}
