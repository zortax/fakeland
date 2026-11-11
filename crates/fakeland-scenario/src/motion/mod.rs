//! Motion curves of input programs: easing functions, pointer paths and scroll velocity curves.

mod easing;
mod error;
mod path;
mod sampler;
mod velocity;

pub use easing::Easing;
pub use error::MotionError;
pub use path::Path;
pub use sampler::PathSampler;
pub use velocity::VelocityCurve;
