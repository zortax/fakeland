//! Triggers: compositor-side event matchers that run actions with no round trip to the driver.
//! A scenario arms its triggers when it starts; each match runs the actions of the trigger, after
//! its delay, until its limit.

mod arm;
mod matcher;

pub use matcher::{Armed, TriggerSet};
