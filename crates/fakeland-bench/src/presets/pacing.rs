//! The `pacing` preset: watch the target for a while and measure its frame pacing.

use fakeland_scenario::Scenario;
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, positive};
use super::spec::Spec;
use crate::analyzers::{Buffers, FramePacing, InputCoalescing, Resources};
use crate::runner::Bench;

const DEFAULTS: Defaults = Defaults {
    iterations: 1,
    warmup: 0,
    settle: Dur::from_millis(500),
};

/// The parameters of `pacing`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PacingParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// How long each iteration watches.
    pub duration: Dur,
    /// A frame is jank when its present interval exceeds this factor times the median.
    pub jank_factor: f64,
}

impl Default for PacingParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            duration: Dur::from_secs(5),
            jank_factor: 1.5,
        }
    }
}

pub(crate) struct Pacing;

impl Spec for Pacing {
    const NAME: &'static str = "pacing";
    const DESCRIPTION: &'static str = "Watches the target without input and measures frame pacing: \
        present intervals, jitter, lows, jank, missed vblanks, cadence and present mode. Needs an \
        app that renders on its own, such as a game, a video or an animation.";
    type Params = PacingParams;

    fn build(p: PacingParams) -> Result<Bench, PresetError> {
        positive(Self::NAME, "duration", p.duration)?;
        let scenario = Scenario::builder("pacing")
            .sleep(p.duration)
            .build_unchecked();
        let bench = Bench::new(Self::NAME)
            .scenario(scenario)
            .analyze(FramePacing::default().jank_factor(p.jank_factor))
            .analyze(Buffers)
            .analyze(InputCoalescing)
            .analyze(Resources);
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
