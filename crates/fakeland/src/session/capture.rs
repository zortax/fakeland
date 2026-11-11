//! Screenshot calls of the session.

use fakeland_scenario::CaptureTarget;

use super::{Session, SessionError};
use crate::capture::{CaptureOptions, CaptureStart, Screenshot};
use crate::util::block_on;

impl Session {
    /// Captures an output, a window or a region. The capture runs in steps clear of output wakes,
    /// and the `max_dim` downscale runs on the calling thread.
    pub async fn screenshot(
        &self,
        target: CaptureTarget,
        options: CaptureOptions,
    ) -> Result<Screenshot, SessionError> {
        let cursor = options.include_cursor;
        let shot = self
            .inner
            .call_with(move |state, reply| {
                state.capture_then(target, cursor, CaptureStart::Clear, move |_, result| {
                    let _ = reply.send(result);
                });
            })
            .await??;
        Ok(shot.downscaled(options.max_dim))
    }

    /// Captures on the calling thread.
    pub fn blocking_screenshot(
        &self,
        target: CaptureTarget,
        options: CaptureOptions,
    ) -> Result<Screenshot, SessionError> {
        block_on(self.screenshot(target, options))
    }
}
