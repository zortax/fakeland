//! Input decoding independent of the Wayland objects: effects, input groups, pointer frame
//! accumulation, delayed application and matching of compositor input ids.

mod delay;
mod effect;
mod group;
mod matcher;
mod pointer_frame;

pub use delay::DelayQueue;
pub use effect::{AxisFrame, AxisSource, InputEffect};
pub use group::{GroupKind, InputGroup};
pub use matcher::MarkMatcher;
pub use pointer_frame::PointerAccumulator;
