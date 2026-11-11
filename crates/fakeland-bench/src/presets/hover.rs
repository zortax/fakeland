//! The `hover-latency` preset: move the pointer onto a point and back, and measure the time to
//! the visible response.

use fakeland_scenario::{Action, PointerAction};
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, WindowPoint};
use super::probe::ProbeParams;
use super::spec::Spec;
use crate::analyzers::{Latency, Resources};
use crate::runner::Bench;

const DEFAULTS: Defaults = Defaults {
    iterations: 3,
    warmup: 1,
    settle: Dur::from_millis(200),
};

/// The parameters of `hover-latency`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct HoverParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The probe settings.
    #[serde(flatten)]
    pub probe: ProbeParams,
    /// The point that reacts to hover.
    pub position: WindowPoint,
    /// The point the pointer returns to between repeats.
    pub away: WindowPoint,
}

impl Default for HoverParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            probe: ProbeParams::default(),
            position: WindowPoint::CENTER,
            away: WindowPoint::fraction(0.02, 0.02),
        }
    }
}

pub(crate) struct Hover;

impl Spec for Hover {
    const NAME: &'static str = "hover-latency";
    const DESCRIPTION: &'static str = "Moves the pointer onto `position` and back to `away` at \
        planned vblank phases and measures input to commit, latch and modeled presentation of \
        the hover response. Needs a widget that changes on hover.";
    type Params = HoverParams;

    fn build(p: HoverParams) -> Result<Bench, PresetError> {
        if !p.position.is_finite() || !p.away.is_finite() {
            return Err(PresetError::params(Self::NAME, "positions must be finite"));
        }
        let seed = p.common.seed();
        let probe = p.probe.clone();
        let (at, away) = (p.position, p.away);
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let w = cx.window();
                probe.scenario(
                    "hover",
                    cx,
                    Action::Pointer(PointerAction::move_to(at.at(w.clone()))),
                    Some(Action::Pointer(PointerAction::move_to(away.at(w)))),
                    seed,
                )
            })
            .analyze(Latency::default())
            .analyze(Resources);
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
