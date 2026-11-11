//! Run validity: compositor wake lateness, host load, missing explicit sync for GPU timing, failed
//! iterations and sample counts. A failed check makes the report invalid; a warning does not.

mod check;
mod config;
mod evaluate;
mod host;

pub use check::{Check, CheckStatus, Validity};
pub use config::ValidityConfig;
pub use evaluate::{LatenessStats, SyncStats, ValidityInputs, evaluate, lateness_of, sync_of};
pub use host::{HostLoad, HostSample};
