//! The `scroll-glide` preset: a constant-velocity scroll with a capture of every presented frame.

use fakeland::CaptureSpec;
use fakeland_scenario::{Position, Scenario, ScrollAxis, ScrollGlide, ScrollSource};
use fakeland_types::{Dur, Logical, Rect};
use fakeland_vision::Axis;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, WindowPoint, positive, positive_f64};
use super::spec::Spec;
use crate::analyzers::{FramePacing, InputCoalescing, ScrollSmoothness};
use crate::runner::Bench;

const DEFAULTS: Defaults = Defaults {
    iterations: 4,
    warmup: 1,
    settle: Dur::from_millis(300),
};

/// The capture settings that scroll and drag presets share.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CaptureParams {
    /// The captured region in window-relative logical coordinates. Default: the whole window.
    pub roi: Option<Rect<Logical>>,
    /// The integer downscale of captured frames.
    pub downscale: u32,
    /// The most frames per iteration.
    pub max_frames: usize,
}

impl Default for CaptureParams {
    fn default() -> Self {
        Self {
            roi: None,
            downscale: 1,
            max_frames: 2000,
        }
    }
}

impl CaptureParams {
    /// Returns the capture of presented frames of the target.
    pub fn presented(&self) -> CaptureSpec {
        let mut spec = CaptureSpec::every_presented(fakeland_scenario::WindowRef::Focused)
            .with_downscale(self.downscale.max(1))
            .with_max_frames(self.max_frames);
        if let Some(r) = self.roi {
            spec = spec.with_roi(r);
        }
        spec
    }
}

/// The parameters of `scroll-glide`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ScrollParams {
    /// The shared parameters.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The capture settings.
    #[serde(flatten)]
    pub capture: CaptureParams,
    /// The pointer position of the scroll.
    pub position: WindowPoint,
    /// The scroll velocity in logical pixels per second; positive scrolls down or right.
    pub velocity: f64,
    /// The glide duration.
    pub duration: Dur,
    /// The scroll device.
    pub source: ScrollSource,
    /// The scroll axis.
    pub axis: ScrollAxis,
    /// The event rate of the glide.
    pub rate_hz: f64,
    /// Odd iterations scroll back, so the content stays in range.
    pub alternate: bool,
    /// The wait after the glide.
    pub tail: Dur,
    /// Content pixels per scroll pixel of the app.
    pub content_factor: f64,
}

impl Default for ScrollParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            capture: CaptureParams::default(),
            position: WindowPoint::CENTER,
            velocity: 1200.0,
            duration: Dur::from_secs(1),
            source: ScrollSource::wheel(true),
            axis: ScrollAxis::Vertical,
            rate_hz: 120.0,
            alternate: true,
            tail: Dur::from_millis(300),
            content_factor: 1.0,
        }
    }
}

/// Returns the vision axis of a scroll axis.
pub(crate) fn vision_axis(axis: ScrollAxis) -> Axis {
    match axis {
        ScrollAxis::Vertical => Axis::Vertical,
        ScrollAxis::Horizontal => Axis::Horizontal,
    }
}

/// Returns the sign of iteration `index` for alternating benches.
pub(crate) fn sign(alternate: bool, index: u32) -> f64 {
    if alternate && index % 2 == 1 {
        -1.0
    } else {
        1.0
    }
}

pub(crate) struct ScrollPreset;

impl Spec for ScrollPreset {
    const NAME: &'static str = "scroll-glide";
    const DESCRIPTION: &'static str = "Scrolls at a constant `velocity` for `duration` with the \
        chosen device and captures every presented frame. Motion estimation on the frames gives \
        the content displacement per refresh against the scroll the events ask for: stalls, \
        double steps, velocity error, start latency and a smoothness score. Needs scrollable \
        content with texture, such as a long page.";
    type Params = ScrollParams;

    fn build(p: ScrollParams) -> Result<Bench, PresetError> {
        positive(Self::NAME, "duration", p.duration)?;
        positive_f64(Self::NAME, "rate_hz", p.rate_hz)?;
        if !p.velocity.is_finite() || p.velocity == 0.0 {
            return Err(PresetError::params(
                Self::NAME,
                "velocity must be finite and nonzero",
            ));
        }
        let q = p.clone();
        let bench = Bench::new(Self::NAME)
            .scenario_fn(move |cx| {
                let at: Position = q.position.at(cx.window());
                let glide =
                    ScrollGlide::constant(q.velocity * sign(q.alternate, cx.index), q.duration)
                        .at(at)
                        .axis(q.axis)
                        .source(q.source)
                        .rate(q.rate_hz);
                Scenario::builder("scroll-glide")
                    .scroll_glide(glide)
                    .sleep(q.tail)
                    .build()
            })
            .capture_frames(p.capture.presented())
            .analyze(ScrollSmoothness {
                axis: vision_axis(p.axis),
                content_factor: p.content_factor,
                ..ScrollSmoothness::default()
            })
            .analyze(FramePacing::default().during_scroll(vision_axis(p.axis)))
            .analyze(InputCoalescing);
        Ok(p.common.apply(bench, DEFAULTS, true))
    }
}
