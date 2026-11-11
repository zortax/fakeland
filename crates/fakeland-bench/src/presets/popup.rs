//! The `popup-open` preset: press a point that opens a popup or menu, then dismiss it.

use fakeland_scenario::{Button, Key, KeyAction, PointerAction, Scenario};
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, WindowPoint, positive};
use super::spec::Spec;
use crate::analyzers::PopupOpen;
use crate::runner::Bench;

const DEFAULTS: Defaults = Defaults {
    iterations: 10,
    warmup: 1,
    settle: Dur::from_millis(300),
};

/// The parameters of `popup-open`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PopupParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The point that opens the popup.
    pub position: WindowPoint,
    /// The button.
    pub button: Button,
    /// How long the popup stays open.
    pub open_for: Dur,
    /// The key that dismisses the popup; `null` clicks `position` again.
    pub dismiss_key: Option<String>,
}

impl Default for PopupParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            position: WindowPoint::CENTER,
            button: Button::Right,
            open_for: Dur::from_secs(1),
            dismiss_key: Some("Escape".into()),
        }
    }
}

pub(crate) struct PopupPreset;

impl Spec for PopupPreset {
    const NAME: &'static str = "popup-open";
    const DESCRIPTION: &'static str = "Clicks `position` with `button` to open a popup, menu or \
        dialog, then dismisses it, and measures press to popup map, first buffer and first \
        presentation. New toplevels count as popups.";
    type Params = PopupParams;

    fn build(p: PopupParams) -> Result<Bench, PresetError> {
        positive(Self::NAME, "open_for", p.open_for)?;
        let q = p.clone();
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let at = q.position.at(cx.window());
                let b = Scenario::builder("popup-open")
                    .step(PointerAction::click(Some(at.clone()), q.button))
                    .sleep(q.open_for);
                let b = match &q.dismiss_key {
                    Some(k) => b.step(KeyAction::tap(Key::name(k))),
                    None => b.step(PointerAction::click(Some(at), q.button)),
                };
                b.sleep(Dur::from_millis(200)).build()
            })
            .analyze(PopupOpen::default());
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
