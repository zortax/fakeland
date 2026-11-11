//! Accessibility trees of the apps in a session: the window frames that the AT-SPI client pairs
//! with app frames, call targets, and the session handle with pointer and keyboard fallbacks.

mod blocking;
mod frames;
mod handle;
mod target;

pub use handle::{Accessibility, ClickOutcome};
pub use target::{A11yTarget, ClickVia, TextMethod};
