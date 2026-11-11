//! The `idle` preset: watch an app that should do nothing, optionally through a suspended span.

use fakeland::CaptureSpec;
use fakeland_scenario::{Scenario, WindowAction, WindowRef};
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, positive};
use super::spec::Spec;
use crate::analyzers::{Buffers, IdleEfficiency, Resources};
use crate::runner::Bench;
use crate::validity::ValidityConfig;

const DEFAULTS: Defaults = Defaults {
    iterations: 1,
    warmup: 0,
    settle: Dur::from_millis(500),
};

/// The parameters of `idle`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct IdleParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// How long each iteration watches.
    pub duration: Dur,
    /// Suspends the window for the middle third of each iteration.
    pub suspend: bool,
    /// Captures every commit for pixel checks and damage accuracy.
    pub capture: bool,
    /// The integer downscale of captured commits.
    pub downscale: u32,
    /// The most captured commits per iteration.
    pub max_frames: usize,
}

impl Default for IdleParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            duration: Dur::from_secs(3),
            suspend: false,
            capture: true,
            downscale: 2,
            max_frames: 1000,
        }
    }
}

pub(crate) struct IdlePreset;

impl Spec for IdlePreset {
    const NAME: &'static str = "idle";
    const DESCRIPTION: &'static str = "Watches the target without input and counts wasted work: \
        commits that change no pixel, damage against the real change, and frame callbacks while \
        idle or suspended. With `capture`, every commit is read back; the readbacks cost \
        compositor time, so late wakes only warn.";
    type Params = IdleParams;

    fn build(p: IdleParams) -> Result<Bench, PresetError> {
        positive(Self::NAME, "duration", p.duration)?;
        let q = p.clone();
        let mut bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                if !q.suspend {
                    return Scenario::builder("idle").sleep(q.duration).build();
                }
                let third = Dur(q.duration.0 / 3);
                let window = cx.window();
                Scenario::builder("idle")
                    .sleep(third)
                    .window(WindowAction::Suspend {
                        window: window.clone(),
                        suspended: true,
                    })
                    .sleep(third)
                    .window(WindowAction::Suspend {
                        window,
                        suspended: false,
                    })
                    .sleep(third)
                    .build()
            })
            .analyze(IdleEfficiency::default())
            .analyze(Buffers)
            .analyze(Resources)
            .validity(ValidityConfig {
                lateness_fails: false,
                ..ValidityConfig::default()
            });
        if p.capture {
            bench = bench.capture_frames(
                CaptureSpec::every_commit(WindowRef::Focused)
                    .with_downscale(p.downscale.max(1))
                    .with_max_frames(p.max_frames),
            );
        }
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
