//! Ready benches. Each preset turns JSON parameters with defaults into a [`Bench`](crate::Bench)
//! with a fitting driver and analyzers. [`list`] describes every preset with the JSON schema of
//! its parameters, and [`by_name`] finds one.

mod click;
mod common;
mod drag;
mod error;
mod fling;
mod hover;
mod idle;
mod info;
mod key;
mod pacing;
mod params;
mod popup;
mod probe;
mod refresh;
mod registry;
mod resize;
mod scale;
mod scroll;
mod spec;
mod startup;
mod throughput;
mod typing;

#[cfg(test)]
mod tests;

pub use common::{CommonParams, DetectorKind, WindowPoint};
pub use error::PresetError;
pub use info::{Preset, PresetInfo};
pub use params::PresetParams;
pub use registry::{NAMES, all, by_name, list};
