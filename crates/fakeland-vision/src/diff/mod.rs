//! Pixel change detection between two frames in a region: full statistics and a fast changed test
//! with early exit.

mod changed;
mod error;
mod pixel;
mod stats;

pub use changed::changed;
pub use error::DiffError;
pub use stats::{DiffStats, diff};

pub(crate) use changed::count_changed_up_to;
pub(crate) use error::check_same_size;
pub(crate) use pixel::max_rgb_delta;
