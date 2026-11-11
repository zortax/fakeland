//! Places the first step of a scenario at a vblank phase.

use fakeland_scenario::{Anchor, Scenario};
use fakeland_types::{Dur, Nanos};

/// Anchors the first step at the next vblank plus `phase` when it starts at once. Other anchors
/// stay as they are.
pub fn anchor_first_step(scenario: &mut Scenario, phase: Nanos) {
    if let Some(step) = scenario.steps.first_mut()
        && step.anchor == Anchor::Immediately
    {
        step.anchor = Anchor::NextVblank {
            output: None,
            phase: Dur(phase),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_scenario::prelude::ms;

    #[test]
    fn only_immediate_first_steps_move() {
        let mut s = Scenario::builder("x").mark("a").mark("b").build().unwrap();
        anchor_first_step(&mut s, Nanos::from_millis(3));
        assert_eq!(
            s.steps[0].anchor,
            Anchor::NextVblank {
                output: None,
                phase: ms(3)
            }
        );
        assert_eq!(s.steps[1].anchor, Anchor::Immediately);
        let mut t = Scenario::builder("y")
            .after(ms(5))
            .mark("a")
            .build()
            .unwrap();
        anchor_first_step(&mut t, Nanos::from_millis(3));
        assert_eq!(t.steps[0].anchor, Anchor::After { after: ms(5) });
    }
}
