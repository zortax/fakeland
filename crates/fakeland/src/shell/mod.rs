//! Window management: toplevel lifecycle, the window policy, configures, popups, focus, mapping,
//! surface scale, window operations and window queries.

mod commit;
mod configure;
mod focus;
mod info;
mod layout;
mod matcher;
mod ops;
mod popups;
mod query;
mod scale;
mod toplevel;
mod tree;
mod windows;

pub use info::{Placement, WindowInfo};
pub use layout::{cascade, initial_layout, placement_rect, window_origin};
pub use matcher::{MatchError, WindowMatcher};
pub use ops::{WindowOp, WindowOpError};
pub use windows::{WindowRecord, Windows};
