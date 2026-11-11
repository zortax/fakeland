//! Expansion of input actions into time-ordered primitive events. Positions stay symbolic, so the
//! compositor resolves each one when the event runs.

mod error;
mod gesture;
mod keyboard;
mod pointer;
mod primitive;
mod program;
mod resize;
mod scroll;
mod ticks;
mod touch;
mod typing;

pub use error::ExpandError;
pub use primitive::{InputProgram, Primitive, TimedPrimitive};
pub use program::expand;
pub use ticks::ticks;
