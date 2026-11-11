//! The hover and click latency probes. Each target runs twice: once with the app-mark detector,
//! which takes the commit that marks a probe input as handled, and once with the pixel detector,
//! which takes the first commit that changes the target region.

use fakeland::scenario::{
    Action, Button, Detector, PhaseSpec, PointerAction, Position, ProbeSpec, Roi,
};
use fakeland::{ProbeReport, Session, SessionError};
use fakeland_types::{Dur, Logical, Point, Rect, WindowId};

use crate::args::{Click, Hover};

/// The detector of a probe run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The app-mark detector.
    AppMark,
    /// The pixel detector.
    Pixels,
}

impl Kind {
    /// The short name.
    pub const fn name(self) -> &'static str {
        match self {
            Self::AppMark => "app_mark",
            Self::Pixels => "pixels",
        }
    }
}

/// One finished probe run.
#[derive(Clone, Debug)]
pub struct ProbeRun {
    /// The target name: hover or click.
    pub target: &'static str,
    /// The detector.
    pub kind: Kind,
    /// The report.
    pub report: ProbeReport,
}

impl ProbeRun {
    /// The label, for example `hover/pixels`.
    pub fn label(&self) -> String {
        format!("{}/{}", self.target, self.kind.name())
    }
}

/// The probe settings that every run shares.
#[derive(Clone, Copy, Debug)]
pub struct Plan {
    /// The window under test.
    pub window: WindowId,
    /// The repeats of each run.
    pub repeats: u32,
}

impl Plan {
    fn at(&self, p: Point<Logical>) -> Position {
        Position::window(self.window, p.x, p.y)
    }

    fn detector(&self, kind: Kind, roi: Rect<Logical>) -> Detector {
        match kind {
            Kind::AppMark => Detector::AppMark { name: None },
            Kind::Pixels => Detector::PixelsChanged {
                roi: Roi::window_rect(self.window, roi),
                threshold: 0.001,
            },
        }
    }

    fn spec(&self, input: Action, detector: Detector, label: String) -> ProbeSpec {
        let mut spec = ProbeSpec::new(input, detector)
            .repeat(self.repeats)
            .phase(PhaseSpec::Sweep { steps: 8 })
            .label(label);
        spec.timeout = Dur::from_millis(1000);
        spec.settle = Dur::from_millis(150);
        spec
    }

    /// Hovers the target and leaves it on alternate repeats.
    pub fn hover(
        &self,
        session: &Session,
        hover: Hover,
        kind: Kind,
    ) -> Result<ProbeRun, SessionError> {
        session.blocking_pointer_move(self.at(hover.away))?;
        let spec = self
            .spec(
                Action::Pointer(PointerAction::move_to(self.at(hover.at))),
                self.detector(kind, hover.roi),
                format!("hover/{}", kind.name()),
            )
            .alternate(Action::Pointer(PointerAction::move_to(self.at(hover.away))));
        Ok(ProbeRun {
            target: "hover",
            kind,
            report: session.blocking_probe(spec)?,
        })
    }

    /// Clicks the target where the pointer already is, with a short press.
    pub fn click(
        &self,
        session: &Session,
        click: Click,
        kind: Kind,
    ) -> Result<ProbeRun, SessionError> {
        session.blocking_pointer_move(self.at(click.at))?;
        let input = Action::Pointer(PointerAction::Click {
            at: None,
            button: Button::default(),
            count: 1,
            interval: Dur::from_millis(10),
            hold: Dur::from_millis(1),
        });
        let spec = self.spec(
            input,
            self.detector(kind, click.roi),
            format!("click/{}", kind.name()),
        );
        Ok(ProbeRun {
            target: "click",
            kind,
            report: session.blocking_probe(spec)?,
        })
    }
}
