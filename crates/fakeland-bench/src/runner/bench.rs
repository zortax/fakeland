//! The bench builder.

use std::sync::Arc;

use fakeland::{CaptureSpec, OutputChange, Reactor, Session};
use fakeland_scenario::{PhaseSpec, Scenario, WindowRef};
use fakeland_types::{Dur, OutputId};
use serde::Serialize;

use super::{BenchRun, Driver, IterationContext, ReactorDriver, ScenarioDriver, Variant};
use crate::analyzer::{Analyzer, AnalyzerSet};
use crate::error::BenchError;
use crate::validity::ValidityConfig;

/// A benchmark over a session: a driver repeated with warmup, settle times and phase plans, the
/// analyzers that read each iteration, and the validity limits.
///
/// ```no_run
/// # use fakeland_bench::{Bench, analyzers::FramePacing};
/// # use fakeland_scenario::{Scenario, WindowRef, prelude::ms};
/// # fn demo(session: &fakeland::Session) -> Result<(), fakeland_bench::BenchError> {
/// let scenario = Scenario::builder("watch").sleep(ms(2000)).build()?;
/// let run = Bench::new("pacing")
///     .target(WindowRef::app_id("org.example.app"))
///     .scenario(scenario)
///     .iterations(3)
///     .analyze(FramePacing::default())
///     .blocking_run(session)?;
/// println!("{}", run.report().to_text());
/// # Ok(()) }
/// ```
#[derive(Clone)]
pub struct Bench {
    pub(crate) name: String,
    pub(crate) preset: Option<String>,
    pub(crate) params: Option<serde_json::Value>,
    pub(crate) target: Option<WindowRef>,
    pub(crate) window_timeout: Dur,
    pub(crate) driver: Option<Arc<dyn Driver>>,
    pub(crate) analyzers: AnalyzerSet,
    pub(crate) warmup: u32,
    pub(crate) iterations: u32,
    pub(crate) settle: Dur,
    pub(crate) quiet: Option<Dur>,
    pub(crate) quiet_timeout: Dur,
    pub(crate) phase: Option<PhaseSpec>,
    pub(crate) seed: u64,
    pub(crate) frames: Vec<CaptureSpec>,
    pub(crate) variants: Vec<Variant>,
    pub(crate) outputs: Vec<(Option<OutputId>, OutputChange)>,
    pub(crate) validity: ValidityConfig,
    pub(crate) keep_warmups: bool,
    pub(crate) stop_on_error: bool,
}

impl std::fmt::Debug for Bench {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bench")
            .field("name", &self.name)
            .field("preset", &self.preset)
            .field("target", &self.target)
            .field("analyzers", &self.analyzers)
            .field("warmup", &self.warmup)
            .field("iterations", &self.iterations)
            .field("variants", &self.variants.len())
            .finish_non_exhaustive()
    }
}

impl Bench {
    /// Builds a bench with one measured iteration, no warmup and a 200 ms settle time.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            preset: None,
            params: None,
            target: None,
            window_timeout: Dur::from_secs(10),
            driver: None,
            analyzers: AnalyzerSet::new(),
            warmup: 0,
            iterations: 1,
            settle: Dur::from_millis(200),
            quiet: None,
            quiet_timeout: Dur::from_secs(5),
            phase: None,
            seed: 0,
            frames: Vec::new(),
            variants: Vec::new(),
            outputs: Vec::new(),
            validity: ValidityConfig::default(),
            keep_warmups: false,
            stop_on_error: false,
        }
    }

    /// Returns the name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Records the preset and its parameters in the report.
    pub fn preset(mut self, name: impl Into<String>, params: impl Serialize) -> Self {
        self.preset = Some(name.into());
        self.params = serde_json::to_value(params).ok();
        self
    }

    /// Sets the window under test. The runner waits for it before the first iteration and
    /// passes it to drivers and analyzers.
    pub fn target(mut self, window: WindowRef) -> Self {
        self.target = Some(window);
        self
    }

    /// Sets how long the runner waits for the target window.
    pub fn window_timeout(mut self, timeout: Dur) -> Self {
        self.window_timeout = timeout;
        self
    }

    /// Runs the same scenario in every iteration.
    pub fn scenario(self, scenario: Scenario) -> Self {
        self.driver(ScenarioDriver::fixed(scenario))
    }

    /// Builds the scenario of each iteration with a closure.
    pub fn scenario_fn<F, E>(self, f: F) -> Self
    where
        F: Fn(&IterationContext) -> Result<Scenario, E> + Send + Sync + 'static,
        E: Into<BenchError>,
    {
        self.driver(ScenarioDriver::from_fn(f))
    }

    /// Installs the reactor that a closure builds in each iteration and waits until it finishes,
    /// at most `timeout`.
    pub fn reactor<F, R>(self, timeout: Dur, make: F) -> Self
    where
        F: Fn(&IterationContext) -> R + Send + Sync + 'static,
        R: Reactor,
        R::Value: Serialize,
    {
        self.driver(ReactorDriver::new(timeout, make))
    }

    /// Sets a custom driver.
    pub fn driver(mut self, driver: impl Driver + 'static) -> Self {
        self.driver = Some(Arc::new(driver));
        self
    }

    /// Adds an analyzer. Each analyzer gives one report section.
    pub fn analyze(mut self, analyzer: impl Analyzer) -> Self {
        self.analyzers.push(analyzer);
        self
    }

    /// Adds every analyzer of a set.
    pub fn analyzers(mut self, set: AnalyzerSet) -> Self {
        for a in set.into_analyzers() {
            self.analyzers.push_dyn(a);
        }
        self
    }

    /// Sets the warmup iterations per variant. Their results do not count.
    pub fn warmup(mut self, n: u32) -> Self {
        self.warmup = n;
        self
    }

    /// Sets the measured iterations per variant.
    pub fn iterations(mut self, n: u32) -> Self {
        self.iterations = n;
        self
    }

    /// Sets the pause before each iteration.
    pub fn settle(mut self, pause: Dur) -> Self {
        self.settle = pause;
        self
    }

    /// Waits before each iteration until the target committed no damage for `quiet`. A target
    /// that never settles only costs the wait timeout.
    pub fn quiet(mut self, quiet: Dur) -> Self {
        self.quiet = Some(quiet);
        self
    }

    /// Plans the vblank phase of the first input of each iteration. Scenario drivers anchor a
    /// first step that starts at once at the next vblank plus this phase.
    pub fn phase(mut self, phase: PhaseSpec) -> Self {
        self.phase = Some(phase);
        self
    }

    /// Sets the seed of iteration seeds and bootstrap intervals.
    pub fn seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    /// Captures frames during each iteration. A spec for the focused window applies to the
    /// target window.
    pub fn capture_frames(mut self, spec: CaptureSpec) -> Self {
        self.frames.push(spec);
        self
    }

    /// Adds a variant. Without variants, the bench runs one unnamed variant.
    pub fn variant(mut self, variant: Variant) -> Self {
        self.variants.push(variant);
        self
    }

    /// Changes the target output before the first iteration. The runner restores every output
    /// at the end.
    pub fn output(mut self, change: OutputChange) -> Self {
        self.outputs.push((None, change));
        self
    }

    /// Changes one output before the first iteration.
    pub fn output_on(mut self, output: OutputId, change: OutputChange) -> Self {
        self.outputs.push((Some(output), change));
        self
    }

    /// Sets the validity limits.
    pub fn validity(mut self, config: ValidityConfig) -> Self {
        self.validity = config;
        self
    }

    /// Keeps the traces of warmup iterations in the run.
    pub fn keep_warmups(mut self, keep: bool) -> Self {
        self.keep_warmups = keep;
        self
    }

    /// Stops at the first iteration whose driver fails.
    pub fn stop_on_error(mut self, stop: bool) -> Self {
        self.stop_on_error = stop;
        self
    }

    /// Returns the measured iterations per variant.
    pub fn measured_iterations(&self) -> u32 {
        self.iterations
    }

    /// Returns the section names in order.
    pub fn analyzer_names(&self) -> Vec<String> {
        self.analyzers.names()
    }

    /// Runs the bench.
    pub async fn run(&self, session: &Session) -> Result<BenchRun, BenchError> {
        super::execute::run(self, session).await
    }

    /// Runs the bench on the calling thread.
    pub fn blocking_run(&self, session: &Session) -> Result<BenchRun, BenchError> {
        fakeland::block_on(self.run(session))
    }
}
