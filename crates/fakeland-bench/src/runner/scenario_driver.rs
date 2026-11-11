//! The scenario driver: builds a scenario per iteration and runs it on the compositor thread.

use fakeland::Session;
use fakeland_scenario::Scenario;

use super::phase::anchor_first_step;
use super::{DriveFuture, Driver, DriverOutput, IterationContext};
use crate::error::BenchError;
use crate::input::{Observation, RunSummary};

type Build = Box<dyn Fn(&IterationContext) -> Result<Scenario, BenchError> + Send + Sync>;

/// Runs one scenario per iteration. With a planned phase, a first step that starts at once moves
/// to the next vblank plus the phase. Each labeled step adds an observation with its label at the
/// time the step finished, so a labeled wait marks the time its condition held.
pub struct ScenarioDriver {
    build: Build,
}

impl ScenarioDriver {
    /// Runs the same scenario in every iteration.
    pub fn fixed(scenario: Scenario) -> Self {
        Self {
            build: Box::new(move |_| Ok(scenario.clone())),
        }
    }

    /// Builds the scenario of each iteration with a closure.
    pub fn from_fn<F, E>(f: F) -> Self
    where
        F: Fn(&IterationContext) -> Result<Scenario, E> + Send + Sync + 'static,
        E: Into<BenchError>,
    {
        Self {
            build: Box::new(move |cx| f(cx).map_err(Into::into)),
        }
    }

    /// Returns the scenario of an iteration with the phase applied.
    pub fn scenario(&self, cx: &IterationContext) -> Result<Scenario, BenchError> {
        let mut s = (self.build)(cx)?;
        if let Some(phase) = cx.phase {
            anchor_first_step(&mut s, phase);
        }
        s.validate()?;
        Ok(s)
    }
}

impl Driver for ScenarioDriver {
    fn drive<'a>(&'a self, session: &'a Session, cx: &'a IterationContext) -> DriveFuture<'a> {
        Box::pin(async move {
            let scenario = self.scenario(cx)?;
            let result = session.run(scenario).await?.await?;
            let observations = result
                .steps
                .iter()
                .filter_map(|s| Some(Observation::new(s.label.clone()?, s.finished)))
                .collect();
            Ok(DriverOutput {
                run: Some(RunSummary::from(&result)),
                error: (!result.ok)
                    .then(|| result.error.clone().unwrap_or_else(|| "failed".into())),
                probes: result.probes,
                frames: result.frames,
                observations,
                ..DriverOutput::default()
            })
        })
    }
}
