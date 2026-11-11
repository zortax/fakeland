//! Rewriting of every position in an action, for example to resolve window frames into output
//! coordinates before expansion.

use super::Action;
use crate::{Gesture, PointerAction, Position, TouchAction};

impl Action {
    /// Replaces every position of the action and of its nested steps and probe actions.
    pub fn map_positions(&mut self, f: &mut dyn FnMut(&Position) -> Position) {
        let opt = |p: &mut Option<Position>, f: &mut dyn FnMut(&Position) -> Position| {
            if let Some(pos) = p {
                *pos = f(pos);
            }
        };
        match self {
            Self::Pointer(PointerAction::MoveTo { pos }) => *pos = f(pos),
            Self::Pointer(PointerAction::Click { at, .. }) => opt(at, f),
            Self::Touch(
                TouchAction::Down { pos, .. }
                | TouchAction::Motion { pos, .. }
                | TouchAction::Tap { pos, .. },
            ) => *pos = f(pos),
            Self::Touch(TouchAction::Glide { path, .. }) => path.map_positions(&mut *f),
            Self::Glide(glide) => glide.path.map_positions(&mut *f),
            Self::ScrollGlide(g) => opt(&mut g.at, f),
            Self::Fling(g) => opt(&mut g.at, f),
            Self::Drag(d) => {
                d.from = f(&d.from);
                d.path.map_positions(&mut *f);
            }
            Self::Gesture(Gesture::Pinch { center, .. }) => *center = f(center),
            Self::Gesture(Gesture::Swipe { at, .. } | Gesture::Hold { at, .. }) => opt(at, f),
            Self::Repeat { steps, .. } => {
                for step in steps {
                    step.action.map_positions(f);
                }
            }
            Self::Probe(probe) => {
                probe.input.map_positions(f);
                if let Some(alt) = &mut probe.alternate {
                    alt.map_positions(f);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Drag, WindowRef};
    use fakeland_types::Dur;

    #[test]
    fn every_position_is_visited() {
        let mut a = Action::Drag(Drag::line(
            Position::center(WindowRef::Any),
            Position::output(10.0, 10.0),
            Dur::from_millis(100),
        ));
        let mut seen = 0;
        a.map_positions(&mut |p| {
            seen += 1;
            Position::output(p.coords().0, p.coords().1)
        });
        assert_eq!(seen, 3);
        let Action::Drag(d) = a else {
            panic!("drag expected");
        };
        assert_eq!(d.from, Position::output(0.5, 0.5));
        assert!(d.path.sampler().is_ok());
    }
}
