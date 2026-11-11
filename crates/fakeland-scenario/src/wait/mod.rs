//! Conditions that the compositor waits for: windows, settled content, frames, pixels and app
//! marks.

mod condition;
mod rgba;
mod until;

pub use condition::PixelCondition;
pub use rgba::Rgba;
pub use until::Wait;
