//! Anchor resolution: when the next step starts.

use fakeland_scenario::{
    Action, Anchor, CaptureTarget, Detector, Position, Roi, Step, Wait, WindowRef,
};
use fakeland_types::{OutputId, Timestamp};

use crate::server::State;

/// When a step starts.
#[derive(Clone, Debug, PartialEq)]
pub enum AnchorPlan {
    /// At a time; a time in the past starts at once.
    At(Timestamp),
    /// When a wait holds.
    Wait(Wait),
}

/// Returns the window that an action targets: the window of its first window position, of its
/// window operation, of its capture or of its probe region.
pub fn action_window(action: &Action) -> Option<WindowRef> {
    match action {
        Action::Window(w) => return Some(w.window().clone()),
        Action::Capture {
            target: CaptureTarget::Window { window },
            ..
        }
        | Action::Capture {
            target:
                CaptureTarget::Roi {
                    roi: Roi::Window { window, .. },
                },
            ..
        } => return Some(window.clone()),
        Action::Probe(spec) => {
            if let Detector::AnyDamage {
                roi: Roi::Window { window, .. },
            }
            | Detector::PixelsChanged {
                roi: Roi::Window { window, .. },
                ..
            }
            | Detector::PixelsMatch {
                roi: Roi::Window { window, .. },
                ..
            } = &spec.detector
            {
                return Some(window.clone());
            }
            return action_window(&spec.input);
        }
        _ => {}
    }
    let mut found = None;
    let mut probe = action.clone();
    probe.map_positions(&mut |p| {
        if found.is_none()
            && let Position::Window { window, .. } | Position::WindowFraction { window, .. } = p
        {
            found = Some(window.clone());
        }
        p.clone()
    });
    found
}

impl State {
    /// Resolves the anchor of a step. `started` is the run start and `prev` the end of the
    /// previous step.
    pub fn plan_anchor(&self, step: &Step, started: Timestamp, prev: Timestamp) -> AnchorPlan {
        match &step.anchor {
            Anchor::Immediately => AnchorPlan::At(prev),
            Anchor::At { at } => AnchorPlan::At(started + at.0),
            Anchor::After { after } => AnchorPlan::At(prev + after.0),
            Anchor::NextVblank { output, phase } => {
                let from = self.now().max(prev);
                let at = self
                    .anchor_output(*output, &step.action)
                    .and_then(|o| self.outputs.get(o))
                    .and_then(|o| o.scheduler.next_vblank_after(from))
                    .map_or(from, |g| g.time + phase.0);
                AnchorPlan::At(at)
            }
            Anchor::OnEvent(wait) => AnchorPlan::Wait(wait.clone()),
        }
    }

    /// Returns the output of a vblank anchor: the given output, else the output of the window
    /// the action targets, else the output of the focused window, else the primary output.
    fn anchor_output(&self, output: Option<OutputId>, action: &Action) -> Option<OutputId> {
        if let Some(o) = output {
            return self.outputs.get(o).map(|o| o.id);
        }
        let window = action_window(action).unwrap_or(WindowRef::Focused);
        self.resolve_window(&window)
            .ok()
            .flatten()
            .or_else(|| self.resolve_window(&WindowRef::Focused).ok().flatten())
            .and_then(|w| self.windows.get(w))
            .map(|w| w.output)
            .or_else(|| self.outputs.primary())
    }
}

#[cfg(test)]
mod tests {
    use fakeland_scenario::{PointerAction, ProbeSpec, WindowAction};
    use fakeland_types::WindowId;

    use super::*;

    #[test]
    fn finds_the_target_window_of_actions() {
        let w = WindowRef::id(WindowId(7));
        let click = Action::Pointer(PointerAction::click(
            Some(Position::center(w.clone())),
            Default::default(),
        ));
        assert_eq!(action_window(&click), Some(w.clone()));
        let on_output = Action::Pointer(PointerAction::move_to(Position::output(1.0, 2.0)));
        assert_eq!(action_window(&on_output), None);
        let close = Action::Window(WindowAction::Close { window: w.clone() });
        assert_eq!(action_window(&close), Some(w.clone()));
        let probe = Action::Probe(ProbeSpec::new(
            on_output,
            Detector::AnyDamage {
                roi: Roi::window(w.clone()),
            },
        ));
        assert_eq!(action_window(&probe), Some(w));
        assert_eq!(action_window(&Action::mark("x")), None);
    }
}
