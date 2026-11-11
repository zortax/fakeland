//! The `fling` preset: a finger fling with exponential decay and a capture of every presented
//! frame.

use fakeland_scenario::{Fling, Scenario, ScrollAxis};
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, WindowPoint, positive, positive_f64};
use super::scroll::{CaptureParams, sign, vision_axis};
use super::spec::Spec;
use crate::analyzers::{FramePacing, ScrollSmoothness};
use crate::runner::Bench;

const DEFAULTS: Defaults = Defaults {
    iterations: 4,
    warmup: 1,
    settle: Dur::from_millis(300),
};

/// The parameters of `fling`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct FlingParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The capture settings.
    #[serde(flatten)]
    pub capture: CaptureParams,
    /// The pointer position.
    pub position: WindowPoint,
    /// The start velocity in logical pixels per second.
    pub velocity: f64,
    /// The time constant of the decay.
    pub time_constant: Dur,
    /// The fling duration.
    pub duration: Dur,
    /// The scroll axis.
    pub axis: ScrollAxis,
    /// The event rate.
    pub rate_hz: f64,
    /// Odd iterations fling back.
    pub alternate: bool,
    /// The wait after the fling.
    pub tail: Dur,
}

impl Default for FlingParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            capture: CaptureParams::default(),
            position: WindowPoint::CENTER,
            velocity: 3000.0,
            time_constant: Dur::from_millis(325),
            duration: Dur::from_secs(1),
            axis: ScrollAxis::Vertical,
            rate_hz: 120.0,
            alternate: true,
            tail: Dur::from_millis(300),
        }
    }
}

pub(crate) struct FlingPreset;

impl Spec for FlingPreset {
    const NAME: &'static str = "fling";
    const DESCRIPTION: &'static str = "Sends a finger fling whose velocity decays from `velocity` \
        with `time_constant`, captures every presented frame and measures how the content \
        follows the decaying scroll.";
    type Params = FlingParams;

    fn build(p: FlingParams) -> Result<Bench, PresetError> {
        positive(Self::NAME, "duration", p.duration)?;
        positive(Self::NAME, "time_constant", p.time_constant)?;
        positive_f64(Self::NAME, "rate_hz", p.rate_hz)?;
        positive_f64(Self::NAME, "velocity", p.velocity.abs())?;
        let q = p.clone();
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let mut fling = Fling::new(q.velocity * sign(q.alternate, cx.index));
                fling.at = Some(q.position.at(cx.window()));
                fling.axis = q.axis;
                fling.time_constant = q.time_constant;
                fling.duration = q.duration;
                fling.rate_hz = q.rate_hz;
                Scenario::builder("fling")
                    .fling(fling)
                    .sleep(q.tail)
                    .build()
            })
            .capture_frames(p.capture.presented())
            .analyze(ScrollSmoothness {
                axis: vision_axis(p.axis),
                ..ScrollSmoothness::default()
            })
            .analyze(FramePacing::default());
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
