//! The `startup` preset: launch an app per iteration and time its startup milestones.

use std::future::Future;
use std::pin::pin;
use std::task::Poll;

use fakeland::{LaunchSpec, Session};
use fakeland_scenario::{
    Action, Detector, PointerAction, Position, ProbeSpec, Roi, Wait, WindowRef,
};
use fakeland_types::Dur;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PresetError;
use super::common::{CommonParams, Defaults, positive};
use super::spec::Spec;
use crate::analyzers::{Resources, Startup, startup::VISUALLY_COMPLETE};
use crate::error::BenchError;
use crate::input::Observation;
use crate::runner::{Bench, DriverOutput, IterationContext, driver_fn, sleep};
use crate::validity::ValidityConfig;

const DEFAULTS: Defaults = Defaults {
    iterations: 5,
    warmup: 1,
    settle: Dur::from_millis(500),
};

/// The first input that shows the app responds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FirstInput {
    /// A pointer move to the window center.
    #[default]
    Move,
    /// A click at the window center.
    Click,
    /// No input.
    None,
}

/// The parameters of `startup`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct StartupParams {
    /// The shared parameters; `target` is unused.
    #[serde(flatten)]
    pub common: CommonParams,
    /// The command line of the app.
    pub command: Vec<String>,
    /// Extra environment variables of the app.
    pub env: Vec<(String, String)>,
    /// How long the app has to map a window.
    pub map_timeout: Dur,
    /// The window counts as visually complete after this many equal frames.
    pub stable_frames: u32,
    /// How long the window has to become stable.
    pub stable_timeout: Dur,
    /// The first input after the window is stable.
    pub first_input: FirstInput,
    /// How long the first input has to show a change.
    pub input_timeout: Dur,
    /// The grace time between SIGTERM and SIGKILL at the end of an iteration.
    pub kill_after: Dur,
}

impl Default for StartupParams {
    fn default() -> Self {
        Self {
            common: CommonParams::filled(DEFAULTS),
            command: Vec::new(),
            env: Vec::new(),
            map_timeout: Dur::from_secs(20),
            stable_frames: 5,
            stable_timeout: Dur::from_secs(10),
            first_input: FirstInput::Move,
            input_timeout: Dur::from_secs(2),
            kill_after: Dur::from_secs(2),
        }
    }
}

pub(crate) struct StartupPreset;

impl Spec for StartupPreset {
    const NAME: &'static str = "startup";
    const DESCRIPTION: &'static str = "Launches `command` in each iteration and times spawn to \
        client connect, first toplevel, first buffer, first presentation, visually complete \
        (`stable_frames` equal frames) and the response to a first input. The app ends after \
        each iteration. The readbacks of the stability wait cost compositor time, so late wakes \
        only warn.";
    type Params = StartupParams;

    fn build(p: StartupParams) -> Result<Bench, PresetError> {
        if p.command.is_empty() {
            return Err(PresetError::params(Self::NAME, "command must not be empty"));
        }
        positive(Self::NAME, "map_timeout", p.map_timeout)?;
        positive(Self::NAME, "stable_timeout", p.stable_timeout)?;
        let q = p.clone();
        let bench = Bench::new(Self::NAME)
            .driver(driver_fn(move |session, cx| launch(session, cx, q.clone())))
            .analyze(Startup::default())
            .analyze(Resources)
            .validity(ValidityConfig {
                lateness_fails: false,
                ..ValidityConfig::default()
            });
        Ok(p.common.apply(bench, DEFAULTS, false))
    }
}

async fn launch(
    session: Session,
    _cx: IterationContext,
    p: StartupParams,
) -> Result<DriverOutput, BenchError> {
    let mut spec = LaunchSpec::from_argv(p.command.iter().cloned())
        .ok_or_else(|| BenchError::Driver("empty command".into()))?;
    for (k, v) in &p.env {
        spec = spec.env(k.clone(), v.clone());
    }
    let app = session.launch(spec).await?;
    let mut out = DriverOutput::default();
    let result = observe(&session, &app, &p, &mut out).await;
    if let Err(e) = result {
        out.error = Some(e.to_string());
    }
    let _ = app.terminate();
    let mut exited = pin!(app.wait());
    let mut grace = pin!(sleep(&session, p.kill_after));
    let ended = std::future::poll_fn(|task| {
        if exited.as_mut().poll(task).is_ready() {
            return Poll::Ready(true);
        }
        match grace.as_mut().poll(task) {
            Poll::Ready(_) => Poll::Ready(false),
            Poll::Pending => Poll::Pending,
        }
    })
    .await;
    if !ended {
        let _ = app.kill();
        app.wait().await;
    }
    Ok(out)
}

async fn observe(
    session: &Session,
    app: &fakeland::App,
    p: &StartupParams,
    out: &mut DriverOutput,
) -> Result<(), BenchError> {
    let window = app.wait_window(p.map_timeout.into()).await?;
    out.target = Some(window.clone());
    let me = WindowRef::id(window.id);
    let stable = session
        .wait(Wait::PixelsStable {
            roi: Roi::window(me.clone()),
            frames: p.stable_frames.max(1),
            timeout: p.stable_timeout,
        })
        .await?;
    out.observations
        .push(Observation::new(VISUALLY_COMPLETE, stable.time));
    let input = match p.first_input {
        FirstInput::Move => Action::Pointer(PointerAction::move_to(Position::center(me.clone()))),
        FirstInput::Click => Action::Pointer(PointerAction::click(
            Some(Position::center(me.clone())),
            Default::default(),
        )),
        FirstInput::None => return Ok(()),
    };
    let mut probe = ProbeSpec::new(
        input,
        Detector::AnyDamage {
            roi: Roi::window(me),
        },
    );
    probe.timeout = p.input_timeout;
    probe.settle = Dur::from_millis(50);
    out.probes.push(session.probe(probe).await?);
    Ok(())
}
