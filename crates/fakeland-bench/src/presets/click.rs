//! The `click-latency` preset: click a point and measure the time to the visible response.

use fakeland_scenario::{Action, Button, PointerAction};
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

/// The parameters of `click-latency`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ClickParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The probe settings.
    #[serde(flatten)]
    pub probe: ProbeParams,
    /// The point to click.
    pub position: WindowPoint,
    /// The button.
    pub button: Button,
    /// Clicks again between repeats to toggle the state back.
    pub toggle: bool,
}

impl Default for ClickParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            probe: ProbeParams::default(),
            position: WindowPoint::CENTER,
            button: Button::Left,
            toggle: true,
        }
    }
}

pub(crate) struct Click;

impl Spec for Click {
    const NAME: &'static str = "click-latency";
    const DESCRIPTION: &'static str = "Clicks `position` at planned vblank phases and measures input \
        to the visible response. With `toggle`, a second click between repeats restores the \
        state, which suits toggles and checkboxes.";
    type Params = ClickParams;

    fn build(p: ClickParams) -> Result<Bench, PresetError> {
        if !p.position.is_finite() {
            return Err(PresetError::params(Self::NAME, "position must be finite"));
        }
        let seed = p.common.seed();
        let probe = p.probe.clone();
        let (at, button, toggle) = (p.position, p.button, p.toggle);
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let pos = at.at(cx.window());
                let click = Action::Pointer(PointerAction::click(Some(pos), button));
                let undo = toggle.then(|| click.clone());
                probe.scenario("click", cx, click, undo, seed)
            })
            .analyze(Latency::default())
            .analyze(Resources);
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
