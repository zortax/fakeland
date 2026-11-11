//! Window operations: listing, waits and window changes.

use fakeland::{ClientInfo, WaitOutput, WindowInfo};
use fakeland_scenario::{Wait, WindowRef};
use fakeland_types::{Nanos, Timestamp};

use super::{Controller, OpError};
use crate::methods::{
    NoParams, WaitFramesParams, WaitSettledParams, WaitWindowParams, WindowOpParams,
};

impl Controller {
    /// Returns every window.
    pub async fn windows(&self, _: NoParams) -> Result<Vec<WindowInfo>, OpError> {
        Ok(self.session().windows().await?)
    }

    /// Returns every connected client.
    pub async fn clients(&self, _: NoParams) -> Result<Vec<ClientInfo>, OpError> {
        Ok(self.session().clients().await?)
    }

    /// Waits for a mapped window.
    pub async fn wait_window(&self, p: WaitWindowParams) -> Result<WindowInfo, OpError> {
        Ok(self.session().wait_window(p.window, p.timeout.0).await?)
    }

    /// Returns the mapped window that a matcher selects now.
    pub async fn resolve_window(&self, window: WindowRef) -> Result<WindowInfo, OpError> {
        self.session()
            .wait_window(window, Nanos::from_millis(1))
            .await
            .map_err(|e| match e {
                fakeland::SessionError::Timeout => {
                    OpError::Session(fakeland::SessionError::NoWindow)
                }
                e => e.into(),
            })
    }

    /// Waits until a window shows no new content for the quiet time.
    pub async fn wait_settled(&self, p: WaitSettledParams) -> Result<WindowInfo, OpError> {
        Ok(self
            .session()
            .wait_settled(p.window, p.quiet.0, p.timeout.0)
            .await?)
    }

    /// Waits for presented frames of a window.
    pub async fn wait_frames(&self, p: WaitFramesParams) -> Result<Timestamp, OpError> {
        if p.n == 0 {
            return Err(OpError::InvalidParams("n must be at least 1".into()));
        }
        Ok(self
            .session()
            .wait_frames(p.window, p.n, p.timeout.0)
            .await?)
    }

    /// Runs a scenario wait.
    pub async fn wait(&self, wait: Wait) -> Result<WaitOutput, OpError> {
        Ok(self.session().wait(wait).await?)
    }

    /// Runs a window operation.
    pub async fn window_op(&self, p: WindowOpParams) -> Result<(), OpError> {
        Ok(self.session().window_op(p.window, p.op).await?)
    }
}
