//! Shared domain types of fakeland: monotonic timestamps and spans, human-readable durations,
//! refresh rates, typed ids, logical and physical geometry, the session clock and a seeded random
//! generator. Every other crate names these types.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod clock;
pub mod dur;
mod geometry;
mod ids;
mod rate;
mod rng;
mod time;

pub use clock::{Clock, FakeClock, MonotonicClock, SharedClock};
pub use dur::{Dur, DurParseError};
pub use geometry::{Logical, Physical, Point, Rect, Size, Space};
pub use ids::{
    BufferId, ClientId, CommitId, InputEventId, OutputId, ProbeId, ProcessId, RunId, SurfaceId,
    WindowId,
};
pub use rate::Hz;
pub use rng::SplitMix64;
pub use time::{Nanos, Timestamp};
