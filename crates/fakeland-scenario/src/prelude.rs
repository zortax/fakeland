//! Everything a Rust bench needs to write scenarios, plus short duration constructors.

pub use crate::*;
pub use fakeland_timing::{CallbackPolicy, Perturbation, TimingConfig, TimingMode};
pub use fakeland_types::{Hz, Logical, Nanos, OutputId, Rect, Size, WindowId};

/// Returns a span in milliseconds.
pub const fn ms(v: i64) -> Dur {
    Dur::from_millis(v)
}

/// Returns a span in microseconds.
pub const fn us(v: i64) -> Dur {
    Dur::from_micros(v)
}

/// Returns a span in fractional seconds.
pub fn secs(v: f64) -> Dur {
    Dur::from_secs_f64(v)
}
