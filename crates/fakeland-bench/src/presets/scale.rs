//! The `scale-switch` preset: switch the output scale and measure how the window answers.

use fakeland_scenario::{OutputAction, Scenario};
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, positive, positive_f64};
use super::spec::Spec;
use crate::analyzers::{Buffers, FramePacing, Resize};
use crate::runner::Bench;

const DEFAULTS: Defaults = Defaults {
    iterations: 6,
    warmup: 0,
    settle: Dur::from_millis(300),
};

/// The parameters of `scale-switch`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ScaleParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The scales; iteration `i` switches to `scales[i % len]`.
    pub scales: Vec<f64>,
    /// The wait after each switch.
    pub wait: Dur,
}

impl Default for ScaleParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            scales: vec![1.5, 2.0, 1.0],
            wait: Dur::from_secs(1),
        }
    }
}

pub(crate) struct ScalePreset;

impl Spec for ScalePreset {
    const NAME: &'static str = "scale-switch";
    const DESCRIPTION: &'static str = "Switches the scale of the target output in each iteration \
        and measures configure to the commit at the new size, frames with a stale size, and \
        whether the buffers match the new scale.";
    type Params = ScaleParams;

    fn build(p: ScaleParams) -> Result<Bench, PresetError> {
        positive(Self::NAME, "wait", p.wait)?;
        if p.scales.is_empty() {
            return Err(PresetError::params(Self::NAME, "scales must not be empty"));
        }
        for s in &p.scales {
            positive_f64(Self::NAME, "scale", *s)?;
        }
        let q = p.clone();
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let scale = q.scales[cx.index as usize % q.scales.len()];
                Scenario::builder("scale-switch")
                    .output(OutputAction::SetScale {
                        output: cx.target.as_ref().map(|w| w.output),
                        scale,
                    })
                    .sleep(q.wait)
                    .build()
            })
            .analyze(Resize::default())
            .analyze(Buffers)
            .analyze(FramePacing::default());
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
