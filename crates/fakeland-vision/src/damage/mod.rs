//! Damage accuracy: the damage a client reports for a commit against the pixels that really
//! changed.

mod accuracy;
mod coverage;

pub use accuracy::{DamageAccuracy, damage_accuracy};
