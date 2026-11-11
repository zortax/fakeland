//! Window matchers.

use fakeland_types::WindowId;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Selects a window when an action runs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WindowRef {
    /// The window with this id.
    Id {
        /// The window id.
        id: WindowId,
    },
    /// The first window with this app id.
    AppId {
        /// The `xdg_toplevel` app id.
        app_id: String,
    },
    /// The first window whose title matches this regular expression.
    TitleRegex {
        /// The regular expression.
        pattern: String,
    },
    /// The window with keyboard focus.
    #[default]
    Focused,
    /// Any mapped window.
    Any,
}

impl WindowRef {
    /// Matches a window id.
    pub fn id(id: WindowId) -> Self {
        Self::Id { id }
    }

    /// Matches an app id.
    pub fn app_id(app_id: impl Into<String>) -> Self {
        Self::AppId {
            app_id: app_id.into(),
        }
    }

    /// Matches a title regular expression.
    pub fn title_regex(pattern: impl Into<String>) -> Self {
        Self::TitleRegex {
            pattern: pattern.into(),
        }
    }
}

impl From<WindowId> for WindowRef {
    fn from(id: WindowId) -> Self {
        Self::id(id)
    }
}
