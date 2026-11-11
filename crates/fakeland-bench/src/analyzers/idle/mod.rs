//! Idle efficiency: commits that change nothing, damage against the real pixel change
//! (over-damage and under-damage), and frame callbacks while idle or suspended.

mod analyzer;
mod damage;
mod report;
mod suspended;

#[cfg(test)]
mod tests;

pub use analyzer::IdleEfficiency;
pub use damage::damage_in_capture;
pub use report::{DamageStats, IdleReport, SuspendStats};
pub use suspended::suspended_spans;
