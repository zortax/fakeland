//! The input of an analyzer: one iteration with its trace, timeline and side results.

use std::collections::BTreeSet;

use fakeland::{FrameCapture, ProbeReport, ProbeResult};
use fakeland_trace::{FrameTimeline, Trace};
use fakeland_types::{Hz, Nanos, OutputId, SurfaceId, Timestamp};

use super::{IterationMeta, Observation, RunSummary, Target};

/// One iteration as analyzers see it. Every field is plain data, so tests build inputs from
/// synthetic traces.
#[derive(Clone, Debug)]
pub struct AnalysisInput {
    /// The iteration metadata.
    pub iteration: IterationMeta,
    /// The trace of the iteration with its live context.
    pub trace: Trace,
    /// The frame timeline of the trace.
    pub timeline: FrameTimeline,
    /// The window under test.
    pub target: Option<Target>,
    /// The probe reports of the iteration in order.
    pub probes: Vec<ProbeReport>,
    /// The frame captures of the iteration.
    pub frames: Vec<FrameCapture>,
    /// The scenario run, for scenario drivers.
    pub run: Option<RunSummary>,
    /// Named times that the driver recorded.
    pub observations: Vec<Observation>,
    /// Values that a reactor or custom driver produced.
    pub values: Vec<serde_json::Value>,
}

impl AnalysisInput {
    /// Builds an input from a trace and builds its timeline.
    pub fn new(trace: Trace) -> Self {
        let timeline = FrameTimeline::build(&trace);
        Self {
            iteration: IterationMeta {
                started: trace.start(),
                finished: trace.end(),
                ..IterationMeta::default()
            },
            trace,
            timeline,
            target: None,
            probes: Vec::new(),
            frames: Vec::new(),
            run: None,
            observations: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Sets the iteration metadata.
    pub fn with_meta(mut self, meta: IterationMeta) -> Self {
        self.iteration = meta;
        self
    }

    /// Sets the target window.
    pub fn with_target(mut self, target: Option<Target>) -> Self {
        self.target = target;
        self
    }

    /// Sets the probe reports.
    pub fn with_probes(mut self, probes: Vec<ProbeReport>) -> Self {
        self.probes = probes;
        self
    }

    /// Sets the frame captures.
    pub fn with_frames(mut self, frames: Vec<FrameCapture>) -> Self {
        self.frames = frames;
        self
    }

    /// Sets the run summary.
    pub fn with_run(mut self, run: Option<RunSummary>) -> Self {
        self.run = run;
        self
    }

    /// Sets the observations.
    pub fn with_observations(mut self, observations: Vec<Observation>) -> Self {
        self.observations = observations;
        self
    }

    /// Sets the driver values.
    pub fn with_values(mut self, values: Vec<serde_json::Value>) -> Self {
        self.values = values;
        self
    }

    /// Returns the surfaces under test: the target window tree, or every surface with a commit.
    pub fn target_surfaces(&self) -> BTreeSet<SurfaceId> {
        match &self.target {
            Some(t) => {
                let mut set = self.timeline.window_surfaces(t.window);
                set.insert(t.surface);
                set
            }
            None => self.timeline.committed_surfaces().collect(),
        }
    }

    /// Returns the surface whose frames represent the target: the target surface with the most
    /// presented frames, then the most commits. A tie goes to the target toplevel, then to the
    /// lowest id.
    pub fn main_surface(&self) -> Option<SurfaceId> {
        let toplevel = self.target.as_ref().map(|t| t.surface);
        self.target_surfaces()
            .into_iter()
            .map(|s| {
                let frames = self.timeline.frames(s);
                let presented = frames.iter().filter(|f| f.is_presented_frame()).count();
                (s, presented, frames.len())
            })
            .filter(|(_, _, commits)| *commits > 0)
            .max_by(|a, b| {
                (a.1, a.2, Some(a.0) == toplevel, std::cmp::Reverse(a.0)).cmp(&(
                    b.1,
                    b.2,
                    Some(b.0) == toplevel,
                    std::cmp::Reverse(b.0),
                ))
            })
            .map(|(s, ..)| s)
            .or(toplevel)
    }

    /// Returns the output under test: the target output, else the first output of the trace.
    pub fn output(&self) -> Option<OutputId> {
        self.target
            .as_ref()
            .map(|t| t.output)
            .or_else(|| self.timeline.outputs().map(|o| o.output).min())
    }

    /// Returns the nominal refresh rate of an output, or of the output under test.
    pub fn refresh(&self, output: Option<OutputId>) -> Option<Hz> {
        let id = output.or_else(|| self.output())?;
        self.timeline.output(id).and_then(|o| o.refresh())
    }

    /// Returns the refresh period of an output: the nominal rate, else the refresh that
    /// presentations report.
    pub fn period(&self, output: Option<OutputId>) -> Option<Nanos> {
        if let Some(hz) = self.refresh(output) {
            return Some(hz.period());
        }
        let id = output.or_else(|| self.output());
        self.timeline
            .all_frames()
            .filter_map(|f| f.presented)
            .find(|p| id.is_none_or(|o| p.output == o))
            .and_then(|p| p.refresh)
    }

    /// Returns the scale of an output, or of the output under test; 1 when unknown.
    pub fn scale(&self, output: Option<OutputId>) -> f64 {
        output
            .or_else(|| self.output())
            .and_then(|o| self.timeline.output(o))
            .map(|o| o.scale)
            .filter(|s| *s > 0.0)
            .unwrap_or(1.0)
    }

    /// Returns the time of the first observation with a name.
    pub fn observation(&self, name: &str) -> Option<Timestamp> {
        self.observations
            .iter()
            .find(|o| o.name == name)
            .map(|o| o.time)
    }

    /// Returns every probe result of the iteration in order.
    pub fn probe_results(&self) -> impl Iterator<Item = &ProbeResult> + '_ {
        self.probes.iter().flat_map(|p| p.results.iter())
    }

    /// Returns the start and end time of the iteration.
    pub fn span(&self) -> (Timestamp, Timestamp) {
        (self.iteration.started, self.iteration.finished)
    }
}
