//! Smithay applied a transaction: the renderer state takes the new buffers, the frame pipeline
//! holds the buffer of each applied content commit until the scheduler releases it, and the shell
//! sees the commit.

use fakeland_trace::Event;
use fakeland_types::{CommitId, OutputId, SurfaceId};
use smithay::backend::renderer::utils::{on_commit_buffer_handler, with_renderer_surface_state};
use smithay::input::pointer::CursorImageStatus;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::{TraversalAction, is_sync_subsurface, with_surface_tree_upward};

use super::{CommitEvent, CommitPhase};
use crate::server::State;

impl State {
    /// Handles a commit that smithay applied.
    pub fn surface_applied(&mut self, surface: &WlSurface) {
        if is_sync_subsurface(surface) {
            return;
        }
        on_commit_buffer_handler::<State>(surface);
        self.hold_applied_buffers(surface);
        self.shell_committed(surface);
        self.space_dirty = true;
    }

    /// Holds the buffer of the newest applied content commit of each surface in the tree.
    fn hold_applied_buffers(&mut self, root: &WlSurface) {
        let mut tree = Vec::new();
        with_surface_tree_upward(
            root,
            (),
            |_, _, _| TraversalAction::DoChildren(()),
            |surface, _, _| tree.push(surface.clone()),
            |_, _, _| true,
        );
        for surface in tree {
            let is_root = surface == *root;
            if !is_root && !is_sync_subsurface(&surface) {
                continue;
            }
            let Some(sid) = self.surfaces.id_of(&surface) else {
                continue;
            };
            let entries = self.surfaces.take_applied(sid, !is_root);
            let newest_buffer = entries
                .iter()
                .rev()
                .find(|e| e.new_buffer)
                .map(|e| e.commit);
            if let Some(last) = newest_buffer {
                let buffer =
                    with_renderer_surface_state(&surface, |s| s.buffer().cloned()).flatten();
                if let Some(buffer) = buffer {
                    self.hold_buffer(last, buffer);
                }
            }
            if !entries.is_empty() {
                let commits: Vec<CommitId> = entries.iter().map(|e| e.commit).collect();
                self.run_commit_hooks(&CommitEvent {
                    phase: CommitPhase::Applied,
                    surface: &surface,
                    id: sid,
                    commits: &commits,
                    newest_buffer,
                });
            }
        }
    }

    fn hold_buffer(&mut self, commit: CommitId, buffer: smithay::backend::renderer::utils::Buffer) {
        if self.frames.released_early.remove(&commit) {
            return;
        }
        self.frames.held.insert(commit, buffer);
    }

    /// Forgets a destroyed surface: its scheduler entries end and its records go.
    pub fn surface_destroyed(&mut self, surface: &WlSurface) {
        let Some(rec) = self.surfaces.remove(surface) else {
            return;
        };
        if let Some(window) = self.windows.by_surface(rec.id) {
            self.forget_window(window);
        }
        if matches!(&self.windows.cursor.image, CursorImageStatus::Surface(s) if s == surface) {
            self.windows.cursor.image = CursorImageStatus::default_named();
        }
        if let Some(out) = rec.output {
            self.detach_surface(out, rec.id);
        }
        self.frames.forget_surface(rec.id);
        self.emit(Event::SurfaceDestroyed { surface: rec.id });
    }

    /// Removes a surface from the scheduler of an output and runs the resulting actions.
    /// The scheduler drops the frame callbacks of a removed surface, so they fire at once here:
    /// a live surface that changes output keeps its client drawing.
    pub fn detach_surface(&mut self, output: OutputId, surface: SurfaceId) {
        let actions = match self.outputs.get_mut(output) {
            Some(o) => o.scheduler.remove_surface(surface),
            None => return,
        };
        self.execute_actions(output, actions);
        let ms = self.now().wayland_millis();
        let left: Vec<CommitId> = self
            .frames
            .pending
            .iter()
            .filter(|(_, p)| p.surface == surface)
            .map(|(id, _)| *id)
            .collect();
        let mut fired = false;
        for id in left {
            if let Some(p) = self.frames.pending.get_mut(&id) {
                for cb in p.callbacks.drain(..) {
                    cb.done(ms);
                    fired = true;
                }
                p.finished = true;
            }
            self.frames.collect(id);
        }
        if fired {
            self.flush_clients();
            self.emit(Event::FrameCallbackSent {
                surface,
                time: self.now(),
            });
        }
    }

    /// Returns the output whose scheduler takes the commits of a surface. A surface follows the
    /// window of its root; other surfaces stay on the output they first used. A change of output
    /// ends the entries on the old one.
    pub fn surface_output(&mut self, surface: &WlSurface, sid: SurfaceId) -> OutputId {
        let previous = self.surfaces.get(sid).and_then(|r| r.output);
        let fallback = self.outputs.primary().unwrap_or(OutputId(0));
        let target = self
            .window_of_surface(surface)
            .and_then(|w| self.windows.get(w))
            .map(|w| w.output)
            .filter(|o| self.outputs.get(*o).is_some())
            .or(previous.filter(|o| self.outputs.get(*o).is_some()))
            .unwrap_or(fallback);
        if previous != Some(target) {
            if let Some(old) = previous {
                self.detach_surface(old, sid);
            }
            if let Some(rec) = self.surfaces.get_mut(sid) {
                rec.output = Some(target);
            }
        }
        target
    }
}
