//! Recording start and stop on the compositor thread: the plan fixes the target, the canvas and
//! the frame rate, the start takes the first picture and puts the first slot on the next vblank,
//! and the stop hands the slots up to now to the writer.

use std::sync::Arc;

use fakeland_scenario::{CaptureTarget, WindowRef};
use fakeland_types::{Nanos, Timestamp};

use super::canvas::fit_factor;
use super::registry::{ActiveRecording, RecordPlan, RecordingInfo, Source};
use super::writer::{FrameQueue, WriterThread};
use super::{Canvas, FramePacer, FrameRate, RecordError, RecordSpec, RecordTarget, RecordingId};
use super::{RecordingReport, WriterOutcome};
use crate::capture::CaptureError;
use crate::server::State;

/// A recording the compositor thread let go of. The writer still needs its end.
#[derive(Debug)]
pub struct Stopped {
    /// The frame queue.
    pub queue: FrameQueue,
    /// The writer thread.
    pub writer: WriterThread,
    /// The report without the writer outcome.
    pub report: RecordingReport,
}

impl Stopped {
    /// Ends the stream, waits for the writer and returns the full report.
    pub fn finish(self) -> RecordingReport {
        self.queue.finish();
        let WriterOutcome { frames, error } = self.writer.join();
        let mut report = self.report;
        report.frames = frames;
        report.duration = report.rate.offset(frames);
        report.error = report.error.or(error);
        report
    }
}

impl Source {
    pub(super) fn target(self) -> CaptureTarget {
        match self {
            Self::Output(o) => CaptureTarget::Output { output: Some(o) },
            Self::Window(w) => CaptureTarget::Window {
                window: WindowRef::id(w),
            },
        }
    }
}

impl State {
    /// Resolves the target, size and rate of a recording.
    pub fn plan_recording(&self, spec: RecordSpec) -> Result<RecordPlan, RecordError> {
        let format = spec.resolved_format()?;
        let source = match &spec.target {
            RecordTarget::Output { output } => {
                Source::Output(self.outputs.resolve(*output).ok_or(RecordError::NoOutput)?)
            }
            RecordTarget::Window { window } => {
                Source::Window(self.resolve_window(window)?.ok_or(RecordError::NoWindow)?)
            }
        };
        let output = self.source_output(source).ok_or(RecordError::NoOutput)?;
        let (w, h) = self
            .capture_size(&source.target(), 1.0)
            .map_err(map_capture)?;
        let factor = fit_factor(f64::from(w), f64::from(h), spec.max_dim);
        let (w, h) = self
            .capture_size(&source.target(), factor)
            .map_err(map_capture)?;
        let rate = match spec.fps {
            Some(hz) => FrameRate::from_hz(hz),
            None => self.outputs.get(output).and_then(|o| {
                o.scheduler
                    .period()
                    .and_then(FrameRate::from_period)
                    .or_else(|| FrameRate::from_hz(o.config.mode.refresh))
            }),
        }
        .ok_or_else(|| RecordError::Format("the frame rate must be positive".into()))?;
        Ok(RecordPlan {
            spec,
            format,
            source,
            canvas: Canvas::of(w, h),
            rate,
            factor,
        })
    }

    /// Starts a planned recording with its writer. The first slot is the next vblank of the
    /// output.
    pub fn arm_recording(
        &mut self,
        plan: RecordPlan,
        queue: FrameQueue,
        writer: WriterThread,
    ) -> Result<RecordingId, Box<(RecordError, Stopped)>> {
        let id = self.recordings.next_id();
        let now = self.now();
        let fail = |e: RecordError, queue, writer, plan: &RecordPlan| {
            let report = empty_report(id, plan, now, now);
            Box::new((
                e,
                Stopped {
                    queue,
                    writer,
                    report,
                },
            ))
        };
        let Some(output) = self.source_output(plan.source) else {
            return Err(fail(RecordError::NoOutput, queue, writer, &plan));
        };
        let image =
            match self.capture_scaled(&plan.source.target(), plan.spec.include_cursor, plan.factor)
            {
                Ok(shot) => shot,
                Err(e) => return Err(fail(map_capture(e), queue, writer, &plan)),
            };
        let origin = self
            .outputs
            .get(output)
            .and_then(|o| o.scheduler.next_vblank_after(now))
            .map_or(now, |g| g.time);
        let observer = self.observe(move |state, record| state.recording_record(id, record));
        self.recordings.active.insert(
            id,
            ActiveRecording {
                pacer: FramePacer::new(origin, plan.rate),
                plan,
                output,
                current: Arc::new(image.image),
                fresh: true,
                dirty: false,
                scheduled: false,
                queue,
                writer,
                observer,
                rendered: 0,
                repeated: 0,
                cost: image.time.since(now),
            },
        );
        Ok(id)
    }

    /// Stops a recording. The slots up to now get the latest picture.
    pub fn stop_recording(&mut self, id: RecordingId) -> Option<Stopped> {
        let now = self.now();
        self.advance_recording(id, now, true);
        let mut rec = self.recordings.active.remove(&id)?;
        self.unobserve(rec.observer);
        let mut report = empty_report(id, &rec.plan, rec.pacer.origin(), now);
        report.rendered = rec.rendered;
        report.repeated = rec.repeated;
        report.dropped = rec.queue.dropped();
        report.capture_cost = rec.cost;
        rec.fresh = false;
        Some(Stopped {
            queue: rec.queue,
            writer: rec.writer,
            report,
        })
    }

    /// Stops every recording and waits for the writers, for shutdown.
    pub fn stop_recordings(&mut self) {
        let ids: Vec<RecordingId> = self.recordings.active.keys().copied().collect();
        for id in ids {
            if let Some(stopped) = self.stop_recording(id) {
                let report = stopped.finish();
                tracing::info!("recording stopped at shutdown: {}", report.to_text());
            }
        }
    }

    /// Returns every running recording.
    pub fn recording_infos(&self) -> Vec<RecordingInfo> {
        let mut list: Vec<RecordingInfo> = self
            .recordings
            .active
            .iter()
            .map(|(id, r)| r.info(*id))
            .collect();
        list.sort_by_key(|r| r.id);
        list
    }

    pub(super) fn source_output(&self, source: Source) -> Option<fakeland_types::OutputId> {
        match source {
            Source::Output(o) => self.outputs.get(o).map(|_| o),
            Source::Window(w) => self.windows.get(w).map(|r| r.output),
        }
    }
}

fn map_capture(e: CaptureError) -> RecordError {
    match e {
        CaptureError::NoOutput => RecordError::NoOutput,
        CaptureError::NoWindow => RecordError::NoWindow,
        CaptureError::Match(m) => RecordError::Match(m),
        other => RecordError::Capture(other),
    }
}

fn empty_report(
    id: RecordingId,
    plan: &RecordPlan,
    first_slot: Timestamp,
    stopped: Timestamp,
) -> RecordingReport {
    RecordingReport {
        id,
        path: plan.spec.path.clone(),
        format: plan.format,
        width: plan.canvas.width,
        height: plan.canvas.height,
        rate: plan.rate,
        first_slot,
        stopped,
        frames: 0,
        rendered: 0,
        repeated: 0,
        dropped: 0,
        capture_cost: Nanos::ZERO,
        duration: Nanos::ZERO,
        error: None,
    }
}
