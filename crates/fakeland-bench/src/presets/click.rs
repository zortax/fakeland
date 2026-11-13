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
    /// The point of the click between repeats, when another element brings the state back.
    /// Default: `position`.
    pub undo_position: Option<WindowPoint>,
    /// The time between the press and the release. Zero releases at once, so an app that acts
    /// on the release and an app that acts on the press start from the same time.
    pub hold: Dur,
}

impl Default for ClickParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            probe: ProbeParams::default(),
            position: WindowPoint::CENTER,
            button: Button::Left,
            toggle: true,
            undo_position: None,
            hold: Dur::from_millis(40),
        }
    }
}

pub(crate) struct Click;

impl Spec for Click {
    const NAME: &'static str = "click-latency";
    const DESCRIPTION: &'static str = "Clicks `position` at planned vblank phases and measures input \
        to the visible response. With `toggle`, a second click between repeats restores the \
        state, which suits toggles and checkboxes; `undo_position` puts that click elsewhere.";
    type Params = ClickParams;

    fn build(p: ClickParams) -> Result<Bench, PresetError> {
        if !p.position.is_finite() {
            return Err(PresetError::params(Self::NAME, "position must be finite"));
        }
        let seed = p.common.seed();
        let probe = p.probe.clone();
        if p.undo_position.is_some_and(|at| !at.is_finite()) {
            return Err(PresetError::params(
                Self::NAME,
                "undo_position must be finite",
            ));
        }
        let (at, button, toggle, hold) = (p.position, p.button, p.toggle, p.hold);
        let back = p.undo_position.unwrap_or(p.position);
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let click_at = |point: WindowPoint| {
                    Action::Pointer(PointerAction::Click {
                        at: Some(point.at(cx.window())),
                        button,
                        count: 1,
                        interval: Dur::from_millis(120),
                        hold,
                    })
                };
                let click = click_at(at);
                let undo = toggle.then(|| click_at(back));
                probe.scenario("click", cx, click, undo, seed)
            })
            .analyze(Latency::default())
            .analyze(Resources);
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
