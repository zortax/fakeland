//! Typing: key-to-glyph latency per key press and the backlog of unhandled keys under bursts.

mod analyzer;
mod keys;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::Typing;
pub use keys::{is_modifier, key_name};
pub use report::{KeyLatency, TypingReport};
