//! The `drag-follow` preset: a pointer drag with a capture of every presented frame.

use fakeland_scenario::{Button, Drag, Scenario};
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, WindowPoint, positive, positive_f64};
use super::scroll::CaptureParams;
use super::spec::Spec;
use crate::analyzers::{DragFollow, FramePacing};
use crate::runner::Bench;

const DEFAULTS: Defaults = Defaults {
    iterations: 4,
    warmup: 1,
    settle: Dur::from_millis(300),
};

/// The parameters of `drag-follow`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DragParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The capture settings.
    #[serde(flatten)]
    pub capture: CaptureParams,
    /// The press point.
    pub from: WindowPoint,
    /// The release point.
    pub to: WindowPoint,
    /// The drag duration.
    pub duration: Dur,
    /// The button.
    pub button: Button,
    /// The motion rate.
    pub rate_hz: f64,
    /// Odd iterations drag back.
    pub alternate: bool,
    /// The wait after the release.
    pub tail: Dur,
}

impl Default for DragParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            capture: CaptureParams::default(),
            from: WindowPoint::fraction(0.3, 0.5),
            to: WindowPoint::fraction(0.7, 0.5),
            duration: Dur::from_millis(600),
            button: Button::Left,
            rate_hz: 125.0,
            alternate: true,
            tail: Dur::from_millis(300),
        }
    }
}

pub(crate) struct DragPreset;

impl Spec for DragPreset {
    const NAME: &'static str = "drag-follow";
    const DESCRIPTION: &'static str = "Drags from `from` to `to` and captures every presented \
        frame; motion estimation gives how far the dragged content trails the pointer in pixels \
        and milliseconds. Suits maps, canvases, sliders and draggable panels.";
    type Params = DragParams;

    fn build(p: DragParams) -> Result<Bench, PresetError> {
        positive(Self::NAME, "duration", p.duration)?;
        positive_f64(Self::NAME, "rate_hz", p.rate_hz)?;
        let q = p.clone();
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let w = cx.window();
                let (a, b) = if q.alternate && cx.index % 2 == 1 {
                    (q.to, q.from)
                } else {
                    (q.from, q.to)
                };
                let mut drag = Drag::line(a.at(w.clone()), b.at(w), q.duration);
                drag.button = q.button;
                drag.rate_hz = q.rate_hz;
                Scenario::builder("drag").drag(drag).sleep(q.tail).build()
            })
            .capture_frames(p.capture.presented())
            .analyze(DragFollow::default())
            .analyze(FramePacing::default());
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
