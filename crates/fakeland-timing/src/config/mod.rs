//! Timing configuration: the refresh mode, the latch offset, the frame-callback policy and the
//! optional perturbation of an output.

mod callbacks;
mod error;
mod mode;
mod perturbation;
mod timing;

pub use callbacks::CallbackPolicy;
pub use error::ConfigError;
pub use mode::TimingMode;
pub use perturbation::Perturbation;
pub use timing::TimingConfig;
