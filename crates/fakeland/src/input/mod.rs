//! Input injection through the smithay seat: pointer motion with hit-tested focus, buttons, axis
//! events by source, keys by keysym or keycode with shift levels, text through text input or a
//! temporary keymap, touch and touchpad gestures. Every event reaches its client at once; its
//! `InputSent` record takes the time after the flush, and the app-mark managers of the client get
//! the input id before the event.

mod error;
mod focus;
mod gesture;
mod inject;
mod keyboard;
mod keymap;
mod pointer;
mod position;
mod sent;
mod state;
mod text;
mod text_inputs;
mod touch;

pub use error::InjectError;
pub use gesture::GestureInput;
pub use keymap::{Chord, KeymapLookup};
pub use pointer::AxisInput;
pub use sent::{PendingSent, SentInput, SentSink};
pub use state::{DeferredMotion, HeldKey, InputState, TempKeymap, TouchPoint};
pub use text_inputs::TextInputs;
