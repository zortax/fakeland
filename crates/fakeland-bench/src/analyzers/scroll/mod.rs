//! Scroll smoothness: the content displacement of each presented frame, from motion estimation on
//! frame captures, against the displacement the injected scroll events ask for times the content
//! gain of the app. The report gives the gain, counts stalls and double steps and gives the
//! velocity error, the scroll start latency and a smoothness score.

mod analyzer;
mod expected;
mod gain;
mod measure;
mod report;
mod score;

#[cfg(test)]
mod tests;

pub use analyzer::ScrollSmoothness;
pub use expected::{ScrollInput, scroll_inputs};
pub use report::{FrameStep, ScrollReport};
pub use score::smoothness_score;
