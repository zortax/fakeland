//! Accessibility tree access over AT-SPI on the session's private bus: the tree of an app or a
//! window, element search by role, name and state, element actions, text and value changes, and
//! a compact text form for agents.
//!
//! [`A11y::connect`] asks the session bus for the accessibility bus and connects there. Calls
//! take the compositor windows as [`WindowFrame`]s, so frames pair with windows and bounds map
//! into the global logical space of the session (see [`mapping`] for the rules and their
//! limits). The API is async on zbus with its own I/O thread, so it runs under any executor;
//! every call has a `blocking_*` twin. [`A11y::render`] prints a tree with short node names such
//! as `n12`, and [`A11y::parse_id`] reads them back.
//!
//! What the toolkits expose:
//!
//! - GTK 4 builds its tree as soon as the bus runs. Buttons offer `click`, text boxes offer
//!   `activate` and EditableText. Widgets also list toolkit actions such as `clipboard.copy`,
//!   which the text form leaves out. GTK 4 leaves CSS margins out of its window coordinates, so
//!   positions can be off by those margins while sizes are right. Its object paths are long, so
//!   the short names help.
//! - Qt builds its tree with `QT_LINUX_ACCESSIBILITY_ALWAYS_ON=1`. Its frame is the client area
//!   inside its own title bar, and the mapping places it there. Actions are `Press`, `Toggle` and
//!   `SetFocus`.
//! - Chromium and Electron expose only their window frame until an assistive technology asks for
//!   more. Pass `--force-renderer-accessibility` on their command line for the full tree. Web
//!   content reports physical pixels at fractional scales, and the mapping converts that
//!   subtree. Action names are `press`, `click` or `doDefault`; entries offer no EditableText, so
//!   text changes go through the keyboard.
//! - AccessKit apps, such as zgui apps, answer `GetRole` without `GetRoleName`, and their bounds
//!   are exact.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod actions;
mod aliases;
mod block_on;
mod client;
mod error;
mod id;
pub mod mapping;
mod matching;
mod node;
mod options;
mod query;
mod render;
mod roles;
mod scope;
mod states;
mod window;

pub use actions::{can_activate, resolve as resolve_action};
pub use aliases::{Aliases, MAX_ALIASES};
pub use block_on::block_on;
pub use client::{
    A11y, AppInfo, BUS_NAME, BusStatus, DEFAULT_CALL_TIMEOUT, FrameInfo, bus_address, bus_status,
};
pub use error::A11yError;
pub use id::{NodeRef, PATH_PREFIX, REGISTRY, ROOT_PATH};
pub use mapping::{Anchor, Mapping};
pub use matching::{FrameCandidate, match_frames};
pub use node::{CoordSpace, Extents, Node, NodeValue};
pub use options::TreeOptions;
pub use query::{Matcher, Query};
pub use render::{
    IdFn, TextOptions, render, render_line, render_list, render_list_with, render_with,
    shown_actions, shown_states,
};
pub use roles::{is_frame, is_generic, matches as role_matches, normalize as normalize_role};
pub use scope::Scope;
pub use states::{has_state, state_names};
pub use window::WindowFrame;
