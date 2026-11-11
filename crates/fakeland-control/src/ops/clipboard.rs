//! Clipboard operations.

use super::{Controller, OpError};
use crate::methods::{ClipboardGetParams, ClipboardSetParams};

impl Controller {
    /// Reads the clipboard text.
    pub async fn clipboard_get(&self, p: ClipboardGetParams) -> Result<String, OpError> {
        Ok(self.session().clipboard_text(p.timeout.0).await?)
    }

    /// Sets the clipboard text.
    pub async fn clipboard_set(&self, p: ClipboardSetParams) -> Result<(), OpError> {
        Ok(self.session().set_clipboard(p.mime, p.text).await?)
    }
}
