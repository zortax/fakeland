//! Captures in steps. The render step starts the readback; the event loop then waits for the
//! readback fence, maps the pack buffer and copies the rows in chunks. Every step starts clear
//! of output wakes, and the event loop dispatches its sources and clients between the steps, so
//! a capture of any size keeps display deadlines.

use std::collections::HashMap;

use fakeland_scenario::CaptureTarget;
use fakeland_trace::Event;
use fakeland_types::{Logical, Nanos, OutputId, Rect, Timestamp, WindowId};
use smithay::reexports::calloop::generic::Generic;
use smithay::reexports::calloop::{Interest, Mode, PostAction};

use super::render::Pending;
use super::timing::plan_pixels;
use super::{CaptureError, Screenshot};
use crate::server::State;

/// The poll period of a fence that has no file descriptor.
const FENCE_POLL: Nanos = Nanos::from_micros(250);

/// When the render step of a capture runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStart {
    /// The render runs at once, so the picture shows the state at the call.
    Now,
    /// The render runs clear of output wakes, at most a few wakes later.
    Clear,
}

/// The callback of a stepped capture.
type CaptureDone = Box<dyn FnOnce(&mut State, Result<Screenshot, CaptureError>)>;

/// One capture between its render and its last copy chunk.
struct Job {
    pending: Pending,
    data: Vec<u8>,
    mapped: bool,
    next_row: u32,
    region: Rect<Logical>,
    scale: f64,
    output: Option<OutputId>,
    window: Option<WindowId>,
    what: String,
    time: Timestamp,
    cost: Nanos,
    done: CaptureDone,
}

/// The stepped captures in flight.
#[derive(Default)]
pub struct CaptureJobs {
    next: u64,
    jobs: HashMap<u64, Job>,
    queued: usize,
}

impl std::fmt::Debug for CaptureJobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CaptureJobs")
            .field("in_flight", &self.jobs.len())
            .finish()
    }
}

impl CaptureJobs {
    /// Returns true while a step waits in an idle callback of the event loop.
    pub fn has_queued(&self) -> bool {
        self.queued > 0
    }
}

impl State {
    /// Captures a target in steps and calls `done` with the result. The compositor thread blocks
    /// for one render step and for copy chunks of about [`super::timing::CHUNK_BUDGET`] each.
    /// The screenshot time is the time of the render. The trace cost is the sum of the steps.
    pub fn capture_then(
        &mut self,
        target: CaptureTarget,
        include_cursor: bool,
        start: CaptureStart,
        done: impl FnOnce(&mut State, Result<Screenshot, CaptureError>) + 'static,
    ) {
        let done: CaptureDone = Box::new(done);
        match start {
            CaptureStart::Now => self.start_job(&target, include_cursor, done),
            CaptureStart::Clear => {
                let cost = match self.plan_capture(&target) {
                    Ok(plan) => {
                        let upload = self.pending_upload_pixels(&plan);
                        self.capture_cost.render(plan_pixels(&plan))
                            + self.capture_cost.upload(upload)
                    }
                    Err(e) => return done(self, Err(e)),
                };
                self.run_when_clear(cost, move |state| {
                    state.start_job(&target, include_cursor, done)
                });
            }
        }
    }

    fn start_job(&mut self, target: &CaptureTarget, include_cursor: bool, done: CaptureDone) {
        let start = self.now();
        let plan = match self.plan_capture(target) {
            Ok(plan) => plan,
            Err(e) => return done(self, Err(e)),
        };
        let uploads = self.pending_upload_pixels(&plan);
        let (pending, upload) = match self.start_plan(&plan, include_cursor, true) {
            Ok(s) => match s.pending {
                Some(p) => (p, s.upload),
                None => return done(self, Err(CaptureError::Empty)),
            },
            Err(e) => return done(self, Err(e)),
        };
        let time = self.now();
        let cost = time.since(start);
        self.capture_cost.observe_upload(uploads, upload);
        self.capture_cost
            .observe_render(plan_pixels(&plan), cost - upload);
        let (w, h) = pending.size();
        let job = Job {
            pending,
            data: Vec::with_capacity(w as usize * h as usize * 4),
            mapped: false,
            next_row: 0,
            region: plan.region,
            scale: plan.scale,
            output: plan.output,
            window: plan.window,
            what: plan.what,
            time,
            cost,
            done,
        };
        let id = self.capture_jobs.next;
        self.capture_jobs.next += 1;
        self.capture_jobs.jobs.insert(id, job);
        self.await_fence(id);
    }

    /// Continues a job when its fence has signaled.
    fn await_fence(&mut self, id: u64) {
        let Some(job) = self.capture_jobs.jobs.get(&id) else {
            return;
        };
        let fence = job.pending.fence();
        if fence.is_reached() {
            return self.queue_step(id);
        }
        let Some(fd) = fence.export() else {
            self.schedule_in(FENCE_POLL, move |state| state.await_fence(id));
            return;
        };
        let source = Generic::new(fd, Interest::READ, Mode::OneShot);
        let inserted = self.loop_handle.insert_source(source, move |_, _, state| {
            state.queue_step(id);
            Ok(PostAction::Remove)
        });
        if let Err(e) = inserted {
            tracing::debug!("fence source failed, polling: {}", e.error);
            self.schedule_in(FENCE_POLL, move |state| state.await_fence(id));
        }
    }

    /// Schedules the next step of a job, the map or a copy chunk, after the event loop has
    /// dispatched its pending sources and clients.
    fn queue_step(&mut self, id: u64) {
        let Some(job) = self.capture_jobs.jobs.get(&id) else {
            return;
        };
        let (w, h) = job.pending.size();
        let mapped = job.mapped;
        let rows = self.capture_cost.chunk_rows(w).min(h - job.next_row);
        let cost = if mapped {
            self.capture_cost.copy(u64::from(rows) * u64::from(w))
        } else {
            self.capture_cost.map(u64::from(w) * u64::from(h))
        };
        self.capture_jobs.queued += 1;
        self.loop_handle.insert_idle(move |state| {
            state.capture_jobs.queued -= 1;
            state.run_when_clear(cost, move |state| {
                if mapped {
                    state.copy_chunk(id, rows);
                } else {
                    state.map_job(id);
                }
            });
        });
    }

    fn map_job(&mut self, id: u64) {
        let Some(mut job) = self.capture_jobs.jobs.remove(&id) else {
            return;
        };
        let start = self.now();
        let mapped = self.map_pending(&job.pending);
        let cost = self.now().since(start);
        if let Err(e) = mapped {
            return (job.done)(self, Err(e));
        }
        let (w, h) = job.pending.size();
        self.capture_cost
            .observe_map(u64::from(w) * u64::from(h), cost);
        job.cost += cost;
        job.mapped = true;
        self.capture_jobs.jobs.insert(id, job);
        self.queue_step(id);
    }

    fn copy_chunk(&mut self, id: u64, rows: u32) {
        let Some(mut job) = self.capture_jobs.jobs.remove(&id) else {
            return;
        };
        let start = self.now();
        let (w, h) = job.pending.size();
        let end = (job.next_row + rows).min(h);
        let copied = self.copy_pending(&job.pending, job.next_row..end, &mut job.data);
        let cost = self.now().since(start);
        if let Err(e) = copied {
            return (job.done)(self, Err(e));
        }
        self.capture_cost
            .observe_copy(u64::from(end - job.next_row) * u64::from(w), cost);
        job.cost += cost;
        job.next_row = end;
        if end < h {
            self.capture_jobs.jobs.insert(id, job);
            return self.queue_step(id);
        }
        self.finish_job(job);
    }

    fn finish_job(&mut self, job: Job) {
        let Job {
            pending,
            data,
            region,
            scale,
            output,
            window,
            what,
            time,
            cost,
            done,
            ..
        } = job;
        let image = match pending.into_image(data) {
            Ok(image) => image,
            Err(e) => return done(self, Err(e)),
        };
        self.emit(Event::Capture { what, cost });
        let shot = Screenshot {
            image,
            region,
            scale: 1.0 / scale,
            output,
            window,
            time,
        };
        done(self, Ok(shot));
    }
}
