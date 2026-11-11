//! The `throughput` preset: an unthrottled output and the frame rate the app reaches on it.

use fakeland::OutputChange;
use fakeland_scenario::Scenario;
use fakeland_timing::TimingConfig;
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, positive};
use super::spec::Spec;
use crate::analyzers::{Buffers, Resources, Throughput};
use crate::runner::Bench;
use crate::validity::ValidityConfig;

const DEFAULTS: Defaults = Defaults {
    iterations: 3,
    warmup: 1,
    settle: Dur::from_millis(500),
};

/// The parameters of `throughput`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ThroughputParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// How long each iteration watches.
    pub duration: Dur,
    /// Switches the target output to unthrottled timing for the bench.
    pub unthrottle: bool,
}

impl Default for ThroughputParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            duration: Dur::from_secs(3),
            unthrottle: true,
        }
    }
}

pub(crate) struct ThroughputPreset;

impl Spec for ThroughputPreset {
    const NAME: &'static str = "throughput";
    const DESCRIPTION: &'static str = "Switches the target output to unthrottled timing, where \
        every frame presents once its fence signals, and measures the commit and present rates \
        and the GPU time from commit to fence ready. The app must render without waiting for \
        frame callbacks, such as `vkcube --present_mode 0`.";
    type Params = ThroughputParams;

    fn build(p: ThroughputParams) -> Result<Bench, PresetError> {
        positive(Self::NAME, "duration", p.duration)?;
        let scenario = Scenario::builder("throughput")
            .sleep(p.duration)
            .build_unchecked();
        let mut bench = Bench::new(Self::NAME)
            .scenario(scenario)
            .analyze(Throughput)
            .analyze(Buffers)
            .analyze(Resources)
            .validity(ValidityConfig {
                lateness_fails: false,
                ..ValidityConfig::default()
            });
        if p.unthrottle {
            bench = bench.output(OutputChange::Timing(TimingConfig::unthrottled()));
        }
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
