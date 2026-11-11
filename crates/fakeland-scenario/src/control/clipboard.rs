//! Clipboard actions.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn text_mime() -> String {
    "text/plain;charset=utf-8".to_owned()
}

/// A clipboard action.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum ClipboardAction {
    /// Offers text as the clipboard selection.
    Set {
        /// The MIME type.
        #[serde(default = "text_mime")]
        mime: String,
        /// The contents.
        text: String,
    },
}

impl ClipboardAction {
    /// Offers plain UTF-8 text.
    pub fn text(text: impl Into<String>) -> Self {
        Self::Set {
            mime: text_mime(),
            text: text.into(),
        }
    }
}
