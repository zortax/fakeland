//! The accessibility call of the session: a client for the accessibility bus of the session bus.

use std::sync::Arc;

use fakeland_a11y::{A11y, A11yError, DEFAULT_CALL_TIMEOUT};

use super::{Session, SessionError};
use crate::a11y::Accessibility;
use crate::config::DbusMode;
use crate::util::block_on;

impl Session {
    /// Connects to the accessibility bus of the session bus. With the private bus that is the
    /// bus of the session; with the host bus it is the bus of the host desktop.
    pub async fn a11y(&self) -> Result<Accessibility, SessionError> {
        let session_bus = match self.config().dbus {
            DbusMode::Private => self.dbus_address().map(str::to_string),
            DbusMode::Host => std::env::var("DBUS_SESSION_BUS_ADDRESS").ok(),
        }
        .ok_or_else(|| A11yError::NoBus("the session has no session bus address".into()))?;
        let client = A11y::connect_with(&session_bus, DEFAULT_CALL_TIMEOUT).await?;
        Ok(Accessibility::new(Arc::downgrade(&self.inner), client))
    }

    /// Connects to the accessibility bus on the calling thread.
    pub fn blocking_a11y(&self) -> Result<Accessibility, SessionError> {
        block_on(self.a11y())
    }
}
