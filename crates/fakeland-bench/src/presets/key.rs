//! The `key-latency` preset: tap a key and measure the time to the visible response.

use fakeland_scenario::{Action, Key, KeyAction};
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults};
use super::probe::ProbeParams;
use super::spec::Spec;
use crate::analyzers::{Latency, Resources};
use crate::runner::Bench;

const DEFAULTS: Defaults = Defaults {
    iterations: 3,
    warmup: 1,
    settle: Dur::from_millis(200),
};

/// The parameters of `key-latency`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct KeyParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The probe settings.
    #[serde(flatten)]
    pub probe: ProbeParams,
    /// The key to tap, as an xkb keysym name.
    pub key: String,
    /// The key that undoes it between repeats; `null` taps nothing.
    pub undo: Option<String>,
}

impl Default for KeyParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            probe: ProbeParams::default(),
            key: "a".into(),
            undo: Some("BackSpace".into()),
        }
    }
}

pub(crate) struct KeyPreset;

impl Spec for KeyPreset {
    const NAME: &'static str = "key-latency";
    const DESCRIPTION: &'static str = "Focuses the target and taps `key` at planned vblank phases, \
        tapping `undo` between repeats, and measures key to visible response. Suits text fields \
        and terminals.";
    type Params = KeyParams;

    fn build(p: KeyParams) -> Result<Bench, PresetError> {
        if p.key.is_empty() {
            return Err(PresetError::params(Self::NAME, "key must not be empty"));
        }
        let seed = p.common.seed();
        let probe = p.probe.clone();
        let (key, undo) = (p.key.clone(), p.undo.clone());
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let tap = |k: &str| Action::Key(KeyAction::tap(Key::name(k)));
                probe.scenario("key", cx, tap(&key), undo.as_deref().map(tap), seed)
            })
            .analyze(Latency::default())
            .analyze(Resources);
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
