//! Session policies: window placement, decorations, compositing, renderer choice and isolation.

use fakeland_types::{Logical, Size};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How new toplevel windows are placed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WindowPolicy {
    /// Each toplevel is maximized and activated, and fills its output.
    #[default]
    Kiosk,
    /// Each toplevel floats at a default size. Successive windows cascade.
    Floating {
        /// The size of a new window.
        default_size: Size<Logical>,
    },
    /// Each toplevel is fullscreen on its output.
    Fullscreen,
}

/// Which side draws window decorations.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Decorations {
    /// Clients draw their own decorations.
    #[default]
    ClientSide,
    /// The compositor claims the decorations, so clients draw none.
    ServerSide,
}

/// When the compositor composites outputs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Composite {
    /// Only screenshots, probes and viewers composite.
    #[default]
    Lazy,
    /// Every presented vblank composites the output, as a real compositor does.
    EveryFrame,
}

/// The renderer that composites and imports client buffers.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RendererChoice {
    /// GLES on the render node, or Pixman when no render node opens.
    #[default]
    Auto,
    /// GLES on the render node. The session fails to start without it.
    Gles,
    /// Pixman on the CPU with shm buffers only.
    Pixman,
}

/// Which D-Bus session bus the children use.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DbusMode {
    /// A private `dbus-daemon` that lives as long as the session.
    #[default]
    Private,
    /// The bus of the calling process.
    Host,
}
