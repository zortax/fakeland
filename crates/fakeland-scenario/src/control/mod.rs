//! Non-input actions: output changes, window management, clipboard contents and captures.

mod capture;
mod clipboard;
mod output;
mod window;

pub use capture::CaptureTarget;
pub use clipboard::ClipboardAction;
pub use output::{OutputAction, Transform};
pub use window::WindowAction;
