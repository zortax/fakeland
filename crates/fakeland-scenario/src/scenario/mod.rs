//! Scenarios: named lists of anchored steps plus triggers, their validation and a fluent builder.

mod action;
mod builder;
mod document;
mod error;
mod patterns;
mod positions;
mod step;
mod validate;

pub use action::Action;
pub use builder::ScenarioBuilder;
pub use document::Scenario;
pub use error::ScenarioError;
pub use step::{Anchor, Step};
