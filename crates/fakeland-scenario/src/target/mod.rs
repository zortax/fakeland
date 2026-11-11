//! Targets of scenario actions: window matchers, symbolic positions and regions of interest. The
//! compositor resolves them when an action runs.

mod position;
mod roi;
mod window;

pub use position::Position;
pub use roi::Roi;
pub use window::WindowRef;
