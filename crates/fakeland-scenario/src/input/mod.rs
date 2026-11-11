//! Input actions of scenarios: pointer, keyboard, touch, text, scroll and gesture programs.

mod button;
mod combo;
mod gesture;
mod glide;
mod key;
mod keyboard;
mod pointer;
mod scroll;
mod touch;
mod typing;

pub use button::Button;
pub use combo::{Combo, ComboError, Modifier};
pub use gesture::Gesture;
pub use glide::{Drag, PointerGlide};
pub use key::{Key, keysym_for_char};
pub use keyboard::KeyAction;
pub use pointer::PointerAction;
pub use scroll::{Fling, ScrollAxis, ScrollGlide, ScrollSource};
pub use touch::TouchAction;
pub use typing::{TextVia, TypingCadence};
