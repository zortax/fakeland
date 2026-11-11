//! The `typing` preset: type text at a word rate and measure key-to-glyph latency.

use fakeland_scenario::{Action, Key, KeyAction, Scenario, TypingCadence, WindowAction};
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, positive, positive_f64};
use super::spec::Spec;
use crate::analyzers::{InputCoalescing, Typing};
use crate::runner::Bench;

const DEFAULTS: Defaults = Defaults {
    iterations: 3,
    warmup: 1,
    settle: Dur::from_millis(300),
};

/// The parameters of `typing`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct TypingParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The text to type.
    pub text: String,
    /// Words per minute, five characters per word.
    pub wpm: f64,
    /// The seeded timing jitter in `0..1` of the key interval.
    pub jitter: f64,
    /// How long each key stays down.
    pub hold: Dur,
    /// The wait after the last key, so its glyph presents.
    pub tail: Dur,
    /// Keys that clear the text after each iteration, such as `["ctrl+a", "BackSpace"]`.
    pub clear: Vec<String>,
}

impl Default for TypingParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            text: "the quick brown fox jumps over the lazy dog".into(),
            wpm: 120.0,
            jitter: 0.2,
            hold: Dur::from_millis(20),
            tail: Dur::from_millis(500),
            clear: Vec::new(),
        }
    }
}

pub(crate) struct TypingPreset;

impl Spec for TypingPreset {
    const NAME: &'static str = "typing";
    const DESCRIPTION: &'static str = "Focuses the target and types `text` at `wpm` with seeded \
        jitter, then measures the latency from each key press to the presentation of the frame \
        that handled it, and the backlog of unhandled keys. Raise `wpm` to test bursts.";
    type Params = TypingParams;

    fn build(p: TypingParams) -> Result<Bench, PresetError> {
        positive_f64(Self::NAME, "wpm", p.wpm)?;
        positive(Self::NAME, "hold", p.hold)?;
        if p.text.is_empty() || !(0.0..1.0).contains(&p.jitter) {
            return Err(PresetError::params(
                Self::NAME,
                "text must not be empty and jitter must be in 0..1",
            ));
        }
        let seed = p.common.seed();
        let q = p.clone();
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let cadence = TypingCadence::wpm(q.wpm)
                    .with_jitter(q.jitter, seed.wrapping_add(u64::from(cx.index)))
                    .with_hold(q.hold);
                let mut b = Scenario::builder("typing")
                    .window(WindowAction::Focus {
                        window: cx.window(),
                    })
                    .type_with(q.text.clone(), cadence)
                    .sleep(q.tail);
                for k in &q.clear {
                    b = if k.contains('+') {
                        b.combo(k)
                    } else {
                        b.step(Action::Key(KeyAction::tap(Key::name(k))))
                    };
                }
                b.build()
            })
            .analyze(Typing)
            .analyze(InputCoalescing);
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
