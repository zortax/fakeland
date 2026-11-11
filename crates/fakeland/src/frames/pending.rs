//! The frame pipeline state: commits the scheduler has not finished, the latch barriers that
//! hold smithay transactions, and the buffers held for presented frames.

use std::collections::{HashMap, HashSet};

use fakeland_types::{CommitId, OutputId, SurfaceId};
use smithay::backend::renderer::utils::Buffer;
use smithay::reexports::wayland_server::Client;
use smithay::reexports::wayland_server::protocol::wl_callback::WlCallback;
use smithay::wayland::compositor::Barrier;
use smithay::wayland::presentation::PresentationFeedbackCallback;

use super::{BufferIds, CommitHooks};

/// One commit in the scheduler.
#[derive(Debug)]
pub struct PendingCommit {
    /// The surface.
    pub surface: SurfaceId,
    /// The output whose scheduler holds the commit.
    pub output: OutputId,
    /// The client, for `blocker_cleared` after the latch.
    pub client: Client,
    /// The frame callbacks of the commit.
    pub callbacks: Vec<WlCallback>,
    /// The presentation feedback objects of the commit.
    pub feedback: Vec<PresentationFeedbackCallback>,
    /// The barrier that holds the smithay transaction until the latch.
    pub latch: Option<Barrier>,
    /// The scheduler presented or discarded the commit.
    pub finished: bool,
}

impl PendingCommit {
    /// Returns true when nothing of the commit waits for the scheduler.
    pub fn is_done(&self) -> bool {
        self.finished && self.callbacks.is_empty() && self.latch.is_none()
    }
}

/// The frame pipeline.
#[derive(Debug, Default)]
pub struct Frames {
    /// Commits by id.
    pub pending: HashMap<CommitId, PendingCommit>,
    /// Buffers held for commits until the scheduler releases them.
    pub held: HashMap<CommitId, Buffer>,
    /// Commits the scheduler released before smithay applied them. Their buffers are never held.
    pub released_early: HashSet<CommitId>,
    /// Session ids of live buffers.
    pub buffer_ids: BufferIds,
    /// Per-commit hooks.
    pub hooks: CommitHooks,
}

impl Frames {
    /// Opens the latch barrier of a commit. Returns the client whose transactions can now apply.
    pub fn release_latch(&mut self, commit: CommitId) -> Option<Client> {
        let pending = self.pending.get_mut(&commit)?;
        let barrier = pending.latch.take()?;
        barrier.signal();
        let client = pending.client.clone();
        self.collect(commit);
        Some(client)
    }

    /// Drops the record of a commit once nothing of it is left.
    pub fn collect(&mut self, commit: CommitId) {
        if self
            .pending
            .get(&commit)
            .is_some_and(PendingCommit::is_done)
        {
            self.pending.remove(&commit);
        }
    }

    /// Drops every record of a surface. Open barriers open so smithay never holds a transaction
    /// of a gone surface.
    pub fn forget_surface(&mut self, surface: SurfaceId) {
        self.pending.retain(|_, p| {
            if p.surface != surface {
                return true;
            }
            if let Some(b) = p.latch.take() {
                b.signal();
            }
            false
        });
    }
}
