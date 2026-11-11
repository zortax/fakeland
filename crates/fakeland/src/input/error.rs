//! Errors of input injection.

use thiserror::Error;

use crate::shell::{MatchError, WindowOpError};

/// An input event that cannot be injected.
#[derive(Debug, Error)]
pub enum InjectError {
    /// The seat has no pointer.
    #[error("the seat has no pointer")]
    NoPointer,
    /// The seat has no keyboard.
    #[error("the seat has no keyboard")]
    NoKeyboard,
    /// The seat has no touch device. Enable it with `InputConfig::touch`.
    #[error("the seat has no touch device")]
    NoTouch,
    /// A position names a window that is not mapped.
    #[error("no mapped window matches the position")]
    NoWindow,
    /// A position names an output that does not exist.
    #[error("no such output")]
    NoOutput,
    /// A window matcher is invalid.
    #[error(transparent)]
    Match(#[from] MatchError),
    /// A key name is not an xkb keysym.
    #[error("unknown key {0:?}")]
    UnknownKey(String),
    /// The keymap has no free keycode for a keysym it lacks.
    #[error("the keymap has no free keycode for {0:?}")]
    NoFreeKeycode(String),
    /// A temporary keymap does not compile.
    #[error("the temporary keymap does not compile")]
    Keymap,
    /// A touch point moves or lifts before it goes down.
    #[error("touch point {0} is not down")]
    TouchNotDown(u32),
    /// A window resize fails.
    #[error(transparent)]
    Window(#[from] WindowOpError),
    /// A coordinate is not finite.
    #[error("the position is not finite")]
    NotFinite,
}
