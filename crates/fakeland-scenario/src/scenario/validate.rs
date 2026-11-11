//! Validation of scenarios: every input action expands, every count and duration is in range,
//! triggers hold only actions that run at once, and every title pattern compiles.

use super::{Action, Anchor, Scenario, ScenarioError, Step};
use crate::{Detector, PhaseSpec, Trigger, TriggerEvent, Wait, expand};

impl Scenario {
    /// Checks that the compositor can run the scenario.
    pub fn validate(&self) -> Result<(), ScenarioError> {
        if let Some(t) = self.timeout {
            positive("timeout", t.0.as_nanos(), "the timeout must be positive")?;
        }
        validate_steps("steps", &self.steps)?;
        for (i, trigger) in self.triggers.iter().enumerate() {
            validate_trigger(&format!("triggers[{i}]"), trigger)?;
        }
        self.validate_patterns()
    }
}

fn invalid(location: &str, message: impl Into<String>) -> ScenarioError {
    ScenarioError::Invalid {
        location: location.to_owned(),
        message: message.into(),
    }
}

fn positive(location: &str, value: i64, message: &str) -> Result<(), ScenarioError> {
    if value > 0 {
        Ok(())
    } else {
        Err(invalid(location, message))
    }
}

fn non_negative(location: &str, value: i64) -> Result<(), ScenarioError> {
    if value >= 0 {
        Ok(())
    } else {
        Err(invalid(location, "the duration must not be negative"))
    }
}

fn validate_steps(location: &str, steps: &[Step]) -> Result<(), ScenarioError> {
    for (i, step) in steps.iter().enumerate() {
        let here = format!("{location}[{i}]");
        validate_anchor(&here, &step.anchor)?;
        validate_action(&here, &step.action)?;
    }
    Ok(())
}

fn validate_anchor(location: &str, anchor: &Anchor) -> Result<(), ScenarioError> {
    match anchor {
        Anchor::Immediately => Ok(()),
        Anchor::At { at } => non_negative(location, at.0.as_nanos()),
        Anchor::After { after } => non_negative(location, after.0.as_nanos()),
        Anchor::NextVblank { phase, .. } => non_negative(location, phase.0.as_nanos()),
        Anchor::OnEvent(wait) => validate_wait(location, wait),
    }
}

fn validate_wait(location: &str, wait: &Wait) -> Result<(), ScenarioError> {
    positive(
        location,
        wait.timeout().0.as_nanos(),
        "the wait timeout must be positive",
    )?;
    match wait {
        Wait::Settled { quiet, .. } => non_negative(location, quiet.0.as_nanos()),
        Wait::Frames { n, .. } => positive(location, i64::from(*n), "frames needs n >= 1"),
        Wait::PixelsStable { frames, .. } => positive(
            location,
            i64::from(*frames),
            "pixels_stable needs frames >= 1",
        ),
        _ => Ok(()),
    }
}

fn validate_action(location: &str, action: &Action) -> Result<(), ScenarioError> {
    expand(action).map_err(|source| ScenarioError::Expand {
        location: location.to_owned(),
        source,
    })?;
    match action {
        Action::Wait(wait) => validate_wait(location, wait),
        Action::Sleep { duration } => non_negative(location, duration.0.as_nanos()),
        Action::Repeat { count, steps } => {
            positive(location, i64::from(*count), "repeat needs count >= 1")?;
            validate_steps(&format!("{location}.steps"), steps)
        }
        Action::Probe(probe) => {
            if !probe.input.is_input() {
                return Err(invalid(location, "a probe input must be an input action"));
            }
            validate_action(&format!("{location}.input"), &probe.input)?;
            if let Some(alt) = &probe.alternate {
                validate_action(&format!("{location}.alternate"), alt)?;
            }
            positive(
                location,
                i64::from(probe.repeat),
                "a probe needs repeat >= 1",
            )?;
            positive(
                location,
                probe.timeout.0.as_nanos(),
                "a probe needs a positive timeout",
            )?;
            non_negative(location, probe.settle.0.as_nanos())?;
            if let PhaseSpec::Sweep { steps: 0 } = probe.phase {
                return Err(invalid(location, "a phase sweep needs steps >= 1"));
            }
            if let PhaseSpec::Fixed { offset } = probe.phase {
                non_negative(location, offset.0.as_nanos())?;
            }
            if let Detector::PixelsChanged { threshold, .. } = probe.detector
                && !(0.0..=1.0).contains(&threshold)
            {
                return Err(invalid(location, "a change threshold lies in 0..=1"));
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn validate_trigger(location: &str, trigger: &Trigger) -> Result<(), ScenarioError> {
    if let TriggerEvent::Vblank { every: 0, .. } = trigger.on {
        return Err(invalid(location, "a vblank trigger needs every >= 1"));
    }
    if let TriggerEvent::RoiChanged { threshold, .. } = trigger.on
        && !(0.0..=1.0).contains(&threshold)
    {
        return Err(invalid(location, "a change threshold lies in 0..=1"));
    }
    if let Some(delay) = trigger.delay {
        non_negative(location, delay.0.as_nanos())?;
    }
    for (i, action) in trigger.then.iter().enumerate() {
        let here = format!("{location}.then[{i}]");
        if matches!(
            action,
            Action::Wait(_) | Action::Sleep { .. } | Action::Repeat { .. } | Action::Probe(_)
        ) {
            return Err(invalid(
                &here,
                "trigger actions run at once; waits, sleeps, repeats and probes belong in steps",
            ));
        }
        validate_action(&here, action)?;
    }
    Ok(())
}
