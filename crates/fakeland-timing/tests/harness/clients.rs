//! Synthetic clients with fixed behavior: render after each callback, commit faster than refresh,
//! queue fifo updates, or pace updates with commit-timing targets.

use fakeland_timing::{Action, Update};
use fakeland_types::{CommitId, Nanos, SurfaceId, Timestamp};

/// A client in the simulation.
pub trait Client {
    /// Returns the next time the client wants to run.
    fn next_wake(&self) -> Option<Timestamp>;
    /// Runs the client and returns its commits. `ids` hands out commit ids.
    fn wake(&mut self, now: Timestamp, ids: &mut u64) -> Vec<Update>;
    /// Shows the client one scheduler action.
    fn observe(&mut self, now: Timestamp, action: &Action);
}

fn next_id(ids: &mut u64) -> CommitId {
    let id = CommitId(*ids);
    *ids += 1;
    id
}

fn is_mine(action: &Action, surface: SurfaceId) -> bool {
    match action {
        Action::Presented { surface: s, .. } | Action::Discarded { surface: s, .. } => {
            *s == surface
        }
        _ => false,
    }
}

/// Renders for `render` after each frame callback; the GPU finishes `gpu` after the commit.
pub struct Renderer {
    pub surface: SurfaceId,
    pub render: Nanos,
    pub gpu: Nanos,
    wake: Option<Timestamp>,
    pub commits: Vec<(CommitId, Timestamp)>,
}

impl Renderer {
    pub fn new(surface: SurfaceId, render: Nanos, first: Timestamp) -> Self {
        Self {
            surface,
            render,
            gpu: Nanos::ZERO,
            wake: Some(first),
            commits: Vec::new(),
        }
    }
}

impl Client for Renderer {
    fn next_wake(&self) -> Option<Timestamp> {
        self.wake
    }

    fn wake(&mut self, now: Timestamp, ids: &mut u64) -> Vec<Update> {
        self.wake = None;
        let id = next_id(ids);
        self.commits.push((id, now));
        vec![Update::new(self.surface, id).ready_at(now + self.gpu)]
    }

    fn observe(&mut self, now: Timestamp, action: &Action) {
        if let Action::FrameCallbacks { callbacks, .. } = action
            && callbacks.iter().any(|c| c.surface == self.surface)
        {
            self.wake = Some(now + self.render);
        }
    }
}

/// Commits every `interval` without frame callbacks, like a mailbox swapchain.
pub struct Mailbox {
    pub surface: SurfaceId,
    pub interval: Nanos,
    next: Timestamp,
    until: Timestamp,
}

impl Mailbox {
    pub fn new(surface: SurfaceId, interval: Nanos, first: Timestamp, until: Timestamp) -> Self {
        Self {
            surface,
            interval,
            next: first,
            until,
        }
    }
}

impl Client for Mailbox {
    fn next_wake(&self) -> Option<Timestamp> {
        (self.next <= self.until).then_some(self.next)
    }

    fn wake(&mut self, now: Timestamp, ids: &mut u64) -> Vec<Update> {
        self.next = now + self.interval;
        vec![Update::new(self.surface, next_id(ids)).callback(false)]
    }

    fn observe(&mut self, _: Timestamp, _: &Action) {}
}

/// Keeps `depth` fifo updates in flight, like a FIFO swapchain over fifo-v1.
pub struct FifoClient {
    pub surface: SurfaceId,
    pub depth: usize,
    in_flight: usize,
    pub budget: usize,
    wake: Option<Timestamp>,
}

impl FifoClient {
    pub fn new(surface: SurfaceId, depth: usize, budget: usize, first: Timestamp) -> Self {
        Self {
            surface,
            depth,
            in_flight: 0,
            budget,
            wake: Some(first),
        }
    }
}

impl Client for FifoClient {
    fn next_wake(&self) -> Option<Timestamp> {
        self.wake
    }

    fn wake(&mut self, _: Timestamp, ids: &mut u64) -> Vec<Update> {
        self.wake = None;
        let mut out = Vec::new();
        while self.in_flight < self.depth && self.budget > 0 {
            self.in_flight += 1;
            self.budget -= 1;
            out.push(
                Update::new(self.surface, next_id(ids))
                    .callback(false)
                    .fifo(true, true),
            );
        }
        out
    }

    fn observe(&mut self, now: Timestamp, action: &Action) {
        if is_mine(action, self.surface) {
            self.in_flight -= 1;
            self.wake = Some(now);
        }
    }
}

/// Paces its frames with commit-timing targets: each commit targets `stride` periods after the
/// last presentation.
pub struct PacedClient {
    pub surface: SurfaceId,
    pub period: Nanos,
    pub stride: i64,
    wake: Option<Timestamp>,
    last_present: Option<Timestamp>,
    pub targets: Vec<Timestamp>,
}

impl PacedClient {
    pub fn new(surface: SurfaceId, period: Nanos, stride: i64, first: Timestamp) -> Self {
        Self {
            surface,
            period,
            stride,
            wake: Some(first),
            last_present: None,
            targets: Vec::new(),
        }
    }
}

impl Client for PacedClient {
    fn next_wake(&self) -> Option<Timestamp> {
        self.wake
    }

    fn wake(&mut self, now: Timestamp, ids: &mut u64) -> Vec<Update> {
        self.wake = None;
        let base = self.last_present.unwrap_or(now);
        let target = base + Nanos(self.period.as_nanos() * self.stride);
        self.targets.push(target);
        vec![
            Update::new(self.surface, next_id(ids))
                .callback(false)
                .target(target),
        ]
    }

    fn observe(&mut self, now: Timestamp, action: &Action) {
        if let Action::Presented { surface, time, .. } = action
            && *surface == self.surface
        {
            self.last_present = Some(*time);
            self.wake = Some(now + Nanos::from_millis(2));
        }
    }
}
