//! App startup: spawn, client connect, first toplevel, first buffer, first presentation,
//! visually complete and first responsive input.

mod analyzer;
mod report;

#[cfg(test)]
mod tests;

pub use analyzer::{RESPONSIVE, Startup, VISUALLY_COMPLETE};
pub use report::StartupReport;
