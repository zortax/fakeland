//! The parameters and scenario of probe presets: an input at a point with an optional undo,
//! repeated at planned vblank phases.

use fakeland_scenario::{Action, PhaseSpec, ProbeSpec, Scenario, WindowAction, WindowRef};
use fakeland_types::{Dur, Logical, Rect};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::DetectorKind;
use crate::error::BenchError;
use crate::runner::IterationContext;

/// The probe settings that latency presets share.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ProbeParams {
    /// The response detector.
    pub detector: DetectorKind,
    /// The detector region in window-relative logical coordinates. Default: the whole window.
    pub roi: Option<Rect<Logical>>,
    /// The fraction of region pixels that must change, for the pixel detector.
    pub threshold: f64,
    /// Probe repeats per iteration.
    pub repeats: u32,
    /// The injection phase. Default: a seeded random phase per repeat.
    pub phase: Option<PhaseSpec>,
    /// The deadline of each repeat.
    pub timeout: Dur,
    /// The quiet time before each repeat.
    pub quiet: Dur,
}

impl Default for ProbeParams {
    fn default() -> Self {
        Self {
            detector: DetectorKind::Auto,
            roi: None,
            threshold: 0.001,
            repeats: 20,
            phase: None,
            timeout: Dur::from_secs(1),
            quiet: Dur::from_millis(100),
        }
    }
}

impl ProbeParams {
    /// Builds the scenario of one iteration: focus the window, then probe.
    pub fn scenario(
        &self,
        name: &str,
        cx: &IterationContext,
        input: Action,
        alternate: Option<Action>,
        seed: u64,
    ) -> Result<Scenario, BenchError> {
        let window: WindowRef = cx.window();
        let phase = self.phase.unwrap_or(PhaseSpec::Random {
            seed: seed ^ u64::from(cx.index).wrapping_mul(0x9e37_79b9),
        });
        let mut probe = ProbeSpec::new(
            input,
            self.detector
                .detector(window.clone(), self.roi, self.threshold),
        )
        .repeat(self.repeats.max(1))
        .phase(phase)
        .label(name);
        probe.timeout = self.timeout;
        probe.settle = self.quiet;
        if let Some(a) = alternate {
            probe = probe.alternate(a);
        }
        Ok(Scenario::builder(name)
            .window(WindowAction::Focus { window })
            .probe(probe)
            .build()?)
    }
}
