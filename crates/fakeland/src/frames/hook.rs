//! The commit hook. Each `wl_surface.commit` gets a commit id, lands in the trace and goes to the
//! scheduler of its output. A content commit gets a latch barrier, so smithay applies it when the
//! scheduler latches or discards it.
//!
//! The scheduler is the single source of truth for fifo barriers and commit-timing targets. The
//! smithay fifo and commit-timing globals run unmanaged: the hook reads their requests, and the
//! latch barrier holds the content until the scheduler takes it. Fence readiness reaches the
//! scheduler through `mark_ready`.

use std::collections::VecDeque;

use fakeland_timing::Update;
use fakeland_trace::{Event, SyncKind};
use fakeland_types::{ClientId, SurfaceId, Timestamp};
use smithay::reexports::wayland_server::Resource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::{
    Barrier, add_blocker, add_pre_commit_hook, is_sync_subsurface, with_states,
};
use smithay::wayland::dmabuf::get_dmabuf;

use super::{
    Collected, CommitEvent, CommitPhase, PendingCommit, SurfaceRecord, buffer_info, collect,
};
use crate::server::{Clients, State};

impl State {
    /// Registers a new surface and its commit hook.
    pub fn surface_created(&mut self, surface: &WlSurface) {
        let id = self.ids.surface();
        let client = surface
            .client()
            .and_then(|c| Clients::id_of(&c))
            .unwrap_or(ClientId(0));
        self.surfaces.insert(SurfaceRecord {
            id,
            wl: surface.clone(),
            output: None,
            applies: VecDeque::new(),
            last_damage: None,
            acked_serial: None,
            commits: 0,
        });
        add_pre_commit_hook::<State, _>(surface, |state, _dh, surface| {
            state.surface_committed(surface);
        });
        self.emit(Event::SurfaceCreated {
            surface: id,
            client,
        });
    }

    /// Records the role of a surface.
    pub fn surface_role(&mut self, surface: &WlSurface, role: &str, parent: Option<&WlSurface>) {
        let Some(id) = self.surfaces.id_of(surface) else {
            return;
        };
        let parent = parent.and_then(|p| self.surfaces.id_of(p));
        self.emit(Event::SurfaceRole {
            surface: id,
            role: role.to_string(),
            parent,
        });
    }

    /// Handles one `wl_surface.commit` before smithay caches the state.
    fn surface_committed(&mut self, surface: &WlSurface) {
        let now = self.now();
        let Some(sid) = self.surfaces.id_of(surface) else {
            return;
        };
        let Some(client) = surface.client() else {
            return;
        };
        let commit = self.ids.commit();
        let collected = with_states(surface, collect);
        let sync_child = is_sync_subsurface(surface);
        let output = self.surface_output(surface, sid);

        let buffer = collected.new_buffer().and_then(|b| {
            let id = self.frames.buffer_ids.id(b, &mut self.ids);
            buffer_info::describe(b, id, collected.buffer_scale, collected.viewport)
        });
        let dmabuf = collected
            .new_buffer()
            .and_then(|b| get_dmabuf(b).ok().cloned());
        let sync = if collected.acquire.is_some() {
            SyncKind::Explicit
        } else if dmabuf.is_some() {
            SyncKind::Implicit
        } else {
            SyncKind::None
        };
        let ready = self.watch_readiness(commit, &collected, dmabuf.as_ref());
        let new_buffer = collected.new_buffer().is_some();

        let latch = (collected.has_content() && !sync_child).then(|| {
            let barrier = Barrier::new(false);
            add_blocker(surface, barrier.clone());
            barrier
        });
        let configure_serial = self.surfaces.get_mut(sid).and_then(|rec| {
            rec.commits += 1;
            rec.applies
                .push_back(super::ApplyEntry { commit, new_buffer });
            if !collected.damage.is_empty() {
                rec.last_damage = Some(now);
            }
            rec.acked_serial.take()
        });

        let mut update = Update::new(sid, commit)
            .content(collected.has_content())
            .callback(!collected.callbacks.is_empty())
            .fifo(collected.fifo.set_barrier, collected.fifo.wait_barrier);
        if !ready {
            update = update.pending();
        }
        if let Some(t) = collected.target {
            update = update.target(t);
        }

        self.emit_commit(sid, commit, &collected, buffer, sync, configure_serial);
        if ready && sync != SyncKind::None {
            self.emit(Event::Ready {
                surface: sid,
                commit,
            });
        }
        let Collected {
            callbacks,
            feedback,
            ..
        } = collected;
        self.frames.pending.insert(
            commit,
            PendingCommit {
                surface: sid,
                output,
                client,
                callbacks,
                feedback,
                latch,
                finished: false,
            },
        );
        if let Some(out) = self.outputs.get_mut(output) {
            out.scheduler.submit(now, update);
        }
        self.run_commit_hooks(&CommitEvent {
            phase: CommitPhase::Committed,
            surface,
            id: sid,
            commits: &[commit],
            newest_buffer: new_buffer.then_some(commit),
        });
    }

    fn emit_commit(
        &mut self,
        surface: SurfaceId,
        commit: fakeland_types::CommitId,
        c: &Collected,
        buffer: Option<fakeland_trace::BufferInfo>,
        sync: SyncKind,
        configure_serial: Option<u32>,
    ) {
        let target: Option<Timestamp> = c.target;
        self.emit(Event::Commit {
            surface,
            commit,
            buffer,
            damage: c.damage.clone(),
            frame_callback: !c.callbacks.is_empty(),
            presentation_feedback: !c.feedback.is_empty(),
            sync,
            fifo_barrier: c.fifo.set_barrier,
            fifo_wait: c.fifo.wait_barrier,
            target,
            configure_serial,
            detached: matches!(c.buffer, Some(None)),
        });
    }
}
