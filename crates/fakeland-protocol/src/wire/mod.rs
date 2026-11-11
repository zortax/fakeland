//! The wire encoding of fakeland_marks_v1 values: 64-bit ids in two 32-bit halves, timestamps
//! in the wp_presentation layout, spans and mark names with their checks.

mod id;
mod name;
mod span;
mod time;

pub use id::{join_u64, split_u64};
pub use name::{MAX_NAME_LEN, NameError, check_name};
pub use span::{SpanError, WireSpan};
pub use time::{TimeError, WireTime};
