//! The `resize-storm` preset: an interactive resize from one size to another.

use fakeland_scenario::{Scenario, WindowAction};
use fakeland_types::{Dur, Logical, Size};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, positive, positive_f64};
use super::spec::Spec;
use crate::analyzers::{Buffers, FramePacing, Resize};
use crate::runner::Bench;

const DEFAULTS: Defaults = Defaults {
    iterations: 3,
    warmup: 1,
    settle: Dur::from_millis(500),
};

/// The parameters of `resize-storm`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ResizeParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The first size.
    pub from: Size<Logical>,
    /// The last size.
    pub to: Size<Logical>,
    /// The time from the first to the last configure.
    pub duration: Dur,
    /// The configure rate.
    pub rate_hz: f64,
    /// The wait after the storm.
    pub tail: Dur,
    /// Maximizes the window again after each iteration.
    pub restore: bool,
}

impl Default for ResizeParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            from: Size::new(640, 480),
            to: Size::new(1200, 800),
            duration: Dur::from_secs(1),
            rate_hz: 60.0,
            tail: Dur::from_millis(500),
            restore: true,
        }
    }
}

pub(crate) struct ResizePreset;

impl Spec for ResizePreset {
    const NAME: &'static str = "resize-storm";
    const DESCRIPTION: &'static str = "Configures the target with a stream of sizes from `from` \
        to `to` at `rate_hz`, like an interactive resize, and measures configure to ack to the \
        commit at the new size and its presentation, frames whose size does not match, and \
        frames dropped during the storm.";
    type Params = ResizeParams;

    fn build(p: ResizeParams) -> Result<Bench, PresetError> {
        positive(Self::NAME, "duration", p.duration)?;
        positive_f64(Self::NAME, "rate_hz", p.rate_hz)?;
        if p.from.w <= 0 || p.from.h <= 0 || p.to.w <= 0 || p.to.h <= 0 {
            return Err(PresetError::params(Self::NAME, "sizes must be positive"));
        }
        let q = p.clone();
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let window = cx.window();
                let mut b = Scenario::builder("resize-storm")
                    .window(WindowAction::ResizeStorm {
                        window: window.clone(),
                        from: q.from,
                        to: q.to,
                        duration: q.duration,
                        rate_hz: q.rate_hz,
                        easing: Default::default(),
                    })
                    .sleep(q.tail);
                if q.restore {
                    b = b.window(WindowAction::Maximize { window }).sleep(q.tail);
                }
                b.build()
            })
            .analyze(Resize::default())
            .analyze(FramePacing::default())
            .analyze(Buffers);
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
