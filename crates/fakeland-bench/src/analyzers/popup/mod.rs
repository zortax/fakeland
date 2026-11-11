//! Popup and menu open: the input that opened a popup, the popup map, its first buffer and its
//! first presentation.

mod analyzer;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::PopupOpen;
pub use report::{PopupRecord, PopupReport};
