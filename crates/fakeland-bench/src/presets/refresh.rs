//! The `refresh-sweep` preset: frame pacing at several refresh rates.

use fakeland::{ModeConfig, OutputChange};
use fakeland_scenario::Scenario;
use fakeland_types::{Dur, Hz};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, positive, positive_f64};
use super::spec::Spec;
use crate::analyzers::{FramePacing, InputCoalescing};
use crate::runner::{Bench, Variant};

const DEFAULTS: Defaults = Defaults {
    iterations: 1,
    warmup: 0,
    settle: Dur::from_millis(500),
};

/// The parameters of `refresh-sweep`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct RefreshParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The refresh rates in Hz, one variant each.
    pub rates: Vec<f64>,
    /// How long each iteration watches.
    pub duration: Dur,
}

impl Default for RefreshParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            rates: vec![60.0, 120.0, 144.0],
            duration: Dur::from_secs(3),
        }
    }
}

/// Returns the variant label of a rate, such as `144Hz` or `59.94Hz`.
pub(crate) fn rate_label(hz: f64) -> String {
    if (hz - hz.round()).abs() < 1e-9 {
        format!("{hz:.0}Hz")
    } else {
        format!("{hz}Hz")
    }
}

pub(crate) struct RefreshPreset;

impl Spec for RefreshPreset {
    const NAME: &'static str = "refresh-sweep";
    const DESCRIPTION: &'static str = "Switches the target output through `rates`, keeping its \
        size, and measures frame pacing at each rate. Each rate is a variant of the report.";
    type Params = RefreshParams;

    fn build(p: RefreshParams) -> Result<Bench, PresetError> {
        positive(Self::NAME, "duration", p.duration)?;
        if p.rates.is_empty() {
            return Err(PresetError::params(Self::NAME, "rates must not be empty"));
        }
        for r in &p.rates {
            positive_f64(Self::NAME, "rate", *r)?;
        }
        let scenario = Scenario::builder("refresh-sweep")
            .sleep(p.duration)
            .build_unchecked();
        let mut bench = Bench::new(Self::NAME)
            .scenario(scenario)
            .analyze(FramePacing::default())
            .analyze(InputCoalescing);
        for r in &p.rates {
            bench = bench.variant(
                Variant::new(rate_label(*r))
                    .output(OutputChange::Mode(ModeConfig::new(0, 0, Hz(*r))))
                    .values(serde_json::json!({ "refresh_hz": r })),
            );
        }
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
