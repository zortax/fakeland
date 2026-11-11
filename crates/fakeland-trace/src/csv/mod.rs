//! CSV export of frame timelines: one row per frame with every stage and derived time.

mod field;
mod frames;

pub use frames::{FRAME_COLUMNS, write_frames};
