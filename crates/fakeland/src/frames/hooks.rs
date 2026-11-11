//! Per-commit hooks. A hook runs on the compositor thread when a surface commits, while the
//! pending double-buffered state is still readable through `with_states`, and again when smithay
//! applies the commit at its latch. Protocol extensions and probes build on it.

use fakeland_types::{CommitId, SurfaceId};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use crate::server::State;

/// When a commit hook runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitPhase {
    /// The client committed. The pending state is readable and the commit is in the trace.
    Committed,
    /// Smithay applied the commits. The current state holds their content.
    Applied,
}

/// What a commit hook sees.
#[derive(Debug)]
pub struct CommitEvent<'a> {
    /// The phase.
    pub phase: CommitPhase,
    /// The surface.
    pub surface: &'a WlSurface,
    /// The session id of the surface.
    pub id: SurfaceId,
    /// The commits: one at commit time, every commit of one apply at apply time.
    pub commits: &'a [CommitId],
    /// The newest of `commits` that attaches a new buffer.
    pub newest_buffer: Option<CommitId>,
}

/// The handle of a commit hook.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CommitHookId(pub u64);

type Hook = Box<dyn FnMut(&mut State, &CommitEvent<'_>)>;

/// Every installed commit hook.
#[derive(Default)]
pub struct CommitHooks {
    list: Vec<(CommitHookId, Hook)>,
    removed: Vec<CommitHookId>,
    next: u64,
}

impl std::fmt::Debug for CommitHooks {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommitHooks")
            .field("len", &self.list.len())
            .finish()
    }
}

impl State {
    /// Installs a commit hook.
    pub fn add_commit_hook(
        &mut self,
        hook: impl FnMut(&mut State, &CommitEvent<'_>) + 'static,
    ) -> CommitHookId {
        let id = CommitHookId(self.frames.hooks.next);
        self.frames.hooks.next += 1;
        self.frames.hooks.list.push((id, Box::new(hook)));
        id
    }

    /// Removes a commit hook.
    pub fn remove_commit_hook(&mut self, id: CommitHookId) {
        self.frames.hooks.list.retain(|(h, _)| *h != id);
        self.frames.hooks.removed.push(id);
    }

    /// Runs every commit hook for one event.
    pub(super) fn run_commit_hooks(&mut self, event: &CommitEvent<'_>) {
        if self.frames.hooks.list.is_empty() {
            return;
        }
        let mut list = std::mem::take(&mut self.frames.hooks.list);
        self.frames.hooks.removed.clear();
        for (id, hook) in &mut list {
            if !self.frames.hooks.removed.contains(id) {
                hook(self, event);
            }
        }
        let removed = std::mem::take(&mut self.frames.hooks.removed);
        list.retain(|(id, _)| !removed.contains(id));
        list.append(&mut self.frames.hooks.list);
        self.frames.hooks.list = list;
    }
}
