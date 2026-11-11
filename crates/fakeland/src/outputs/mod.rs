//! Faked outputs: identity, modes, scale, transform, position, power and the display scheduler of
//! each output, plus runtime changes and hotplug.

mod change;
mod fake;
mod manage;
mod registry;

pub use change::{OutputChange, OutputState};
pub use fake::{FakeOutput, smithay_transform};
pub use manage::OutputError;
pub use registry::Outputs;
