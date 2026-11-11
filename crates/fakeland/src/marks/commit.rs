//! Applies the pending marks of a surface at its commit. Each mark becomes an `AppMark` record
//! with the commit id, after the `Commit` record of that commit.

use fakeland_trace::{AppMarkKind, Event};
use fakeland_types::{CommitId, SurfaceId};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use super::with_slot;
use crate::frames::CommitPhase;
use crate::server::State;

impl State {
    /// Installs the commit hook that applies surface marks.
    pub fn install_marks_hook(&mut self) {
        self.add_commit_hook(|state, event| {
            if event.phase != CommitPhase::Committed {
                return;
            }
            if let Some(&commit) = event.commits.first() {
                state.apply_surface_marks(event.surface, event.id, commit);
            }
        });
    }

    /// Takes the pending marks of a surface and records them for a commit.
    fn apply_surface_marks(&mut self, surface: &WlSurface, sid: SurfaceId, commit: CommitId) {
        let pending = with_slot(surface, |slot| std::mem::take(&mut slot.pending));
        if pending.is_empty() {
            return;
        }
        let mut marks = Vec::new();
        if let Some(frame_id) = pending.frame {
            marks.push(AppMarkKind::Frame { frame_id });
        }
        marks.extend(
            pending
                .inputs
                .into_iter()
                .map(|input| AppMarkKind::InputHandled { input }),
        );
        marks.extend(
            pending
                .spans
                .into_iter()
                .map(|(name, begin, end)| AppMarkKind::Span { name, begin, end }),
        );
        marks.extend(
            pending
                .names
                .into_iter()
                .map(|name| AppMarkKind::Mark { name }),
        );
        for mark in marks {
            self.emit(Event::AppMark {
                surface: Some(sid),
                commit: Some(commit),
                mark,
            });
        }
    }
}
