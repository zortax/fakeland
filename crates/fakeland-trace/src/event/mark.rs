//! App marks: instrumentation that a client attaches to a surface commit.

use fakeland_types::{InputEventId, Timestamp};
use serde::{Deserialize, Serialize};

/// One app mark. Marks are double-buffered surface state and apply on commit.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AppMarkKind {
    /// The app frame id of the commit.
    Frame {
        /// The app frame id.
        frame_id: u64,
    },
    /// The commit shows the effect of an input event.
    InputHandled {
        /// The input event.
        input: InputEventId,
    },
    /// A phase of the client work, for example CPU layout or GPU render.
    Span {
        /// The phase name.
        name: String,
        /// The start of the phase.
        begin: Timestamp,
        /// The end of the phase.
        end: Timestamp,
    },
    /// A named point.
    Mark {
        /// The mark name.
        name: String,
    },
}
