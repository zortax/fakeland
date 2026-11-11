//! Drag follow: how far the dragged content trails the pointer, in logical pixels and in
//! milliseconds, from motion estimation on frame captures and the injected pointer path.

mod analyzer;
mod path;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::DragFollow;
pub use path::PointerPath;
pub use report::{DragFollowReport, FollowSample};
