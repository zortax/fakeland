//! Viewer calls of the session.

use super::{Session, SessionError};
use crate::util::block_on;

impl Session {
    /// Returns true while the nested viewer runs. A closed viewer window ends the viewer, and
    /// the session goes on headless.
    pub async fn viewer_running(&self) -> Result<bool, SessionError> {
        #[cfg(feature = "viewer")]
        let running = self.inner.call(|state| state.viewer_running()).await?;
        #[cfg(not(feature = "viewer"))]
        let running = false;
        Ok(running)
    }

    /// Returns true while the nested viewer runs, on the calling thread.
    pub fn blocking_viewer_running(&self) -> Result<bool, SessionError> {
        block_on(self.viewer_running())
    }
}
