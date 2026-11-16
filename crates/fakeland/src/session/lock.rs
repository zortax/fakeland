//! Session lock calls of the session: a forced unlock that recovers a session whose lock client
//! hangs or died.

use super::{Session, SessionError};
use crate::util::block_on;

impl Session {
    /// Unlocks the session without the lock client. Returns false when the session is not
    /// locked. The lock client keeps its lock object and sees the session locked.
    pub async fn force_unlock(&self) -> Result<bool, SessionError> {
        self.inner.call(|state| state.force_unlock()).await
    }

    /// Unlocks the session without the lock client, on the calling thread.
    pub fn blocking_force_unlock(&self) -> Result<bool, SessionError> {
        block_on(self.force_unlock())
    }
}
