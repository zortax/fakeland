//! Clipboard methods.

use fakeland_types::Dur;
use serde::{Deserialize, Serialize};

use super::defaults;
use super::method::method;

/// A clipboard read.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ClipboardGetParams {
    /// The longest wait for the selection owner.
    #[serde(default = "defaults::clipboard_timeout")]
    pub timeout: Dur,
}

impl Default for ClipboardGetParams {
    fn default() -> Self {
        Self {
            timeout: defaults::clipboard_timeout(),
        }
    }
}

/// A clipboard write. The compositor owns the selection afterwards.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClipboardSetParams {
    /// The text.
    pub text: String,
    /// The MIME type.
    #[serde(default = "defaults::text_mime")]
    pub mime: String,
}

method!(
    /// Reads the clipboard text.
    ClipboardGet, "clipboard_get", ClipboardGetParams => String
);
method!(
    /// Sets the clipboard text.
    ClipboardSet, "clipboard_set", ClipboardSetParams => ()
);
