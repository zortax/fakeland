//! Execution of scheduler actions on Wayland: latch barriers open, presentation feedback goes
//! out with the vblank model time, held buffers drop at release, and frame callbacks fire with the
//! vblank time. Every action lands in the trace. A presentation that shows a new buffer feeds the
//! presented frame captures.

use std::time::Duration;

use fakeland_timing::{Action, FrameCallback, PresentFlags, TimingMode};
use fakeland_trace::Event;
use fakeland_types::{CommitId, Nanos, OutputId, SurfaceId, Timestamp};
use smithay::reexports::wayland_protocols::wp::presentation_time::server::wp_presentation_feedback::Kind;
use smithay::wayland::presentation::Refresh;

use crate::config::Composite;
use crate::server::{ClientState, State};

impl State {
    /// Executes the actions of one scheduler in order.
    pub fn execute_actions(&mut self, output: OutputId, actions: Vec<Action>) {
        let mut sent: Vec<SurfaceId> = Vec::new();
        let mut callback_time = None;
        for action in actions {
            match action {
                Action::Vblank {
                    seq,
                    ideal,
                    woke,
                    dropped,
                } => self.on_vblank(output, seq, ideal, woke, dropped),
                Action::Latched {
                    surface,
                    commit,
                    seq,
                } => {
                    self.emit(Event::Latched {
                        surface,
                        commit,
                        output,
                        seq,
                    });
                    self.open_latch(commit);
                }
                Action::Presented {
                    surface,
                    commit,
                    seq,
                    time,
                    refresh,
                    flags,
                    content,
                } => {
                    self.on_presented(output, surface, commit, seq, time, refresh, flags);
                    if let Some(content) = content {
                        self.capture_presented(surface, content, time, seq);
                    }
                }
                Action::Discarded { surface, commit } => {
                    if let Some(p) = self.frames.pending.get_mut(&commit) {
                        p.finished = true;
                        for fb in p.feedback.drain(..) {
                            fb.discarded();
                        }
                    }
                    self.emit(Event::Discarded { surface, commit });
                    self.open_latch(commit);
                    self.frames.collect(commit);
                }
                Action::Released { surface, commit } => {
                    self.release_buffer(surface, commit);
                    self.emit(Event::Released { surface, commit });
                }
                Action::FrameCallbacks { time, callbacks } => {
                    self.fire_callbacks(time, &callbacks, &mut sent);
                    callback_time = Some(time);
                }
                Action::BarrierCleared { .. } => {}
            }
        }
        if let Some(time) = callback_time {
            self.flush_clients();
            let t = self.now();
            for surface in sent {
                self.emit_at(t, Event::FrameCallbackSent { surface, time });
            }
        }
    }

    fn on_vblank(
        &mut self,
        output: OutputId,
        seq: u64,
        ideal: Timestamp,
        woke: Timestamp,
        dropped: bool,
    ) {
        self.emit_at(
            woke,
            Event::Vblank {
                output,
                seq,
                ideal,
                woke,
                dropped,
            },
        );
        if !dropped && self.config.composite == Composite::EveryFrame {
            self.composite_output(output);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn on_presented(
        &mut self,
        output: OutputId,
        surface: SurfaceId,
        commit: CommitId,
        seq: u64,
        time: Timestamp,
        refresh: Option<Nanos>,
        flags: PresentFlags,
    ) {
        let refresh_kind = match (
            refresh,
            self.outputs.get(output).map(|o| &o.scheduler.config().mode),
        ) {
            (Some(r), Some(TimingMode::Vrr { .. })) => Refresh::variable(r.to_duration()),
            (Some(r), _) => Refresh::fixed(r.to_duration()),
            (None, _) => Refresh::Unknown,
        };
        let smithay_output = self.outputs.get(output).map(|o| o.output.clone());
        if let Some(p) = self.frames.pending.get_mut(&commit) {
            p.finished = true;
            let feedback = std::mem::take(&mut p.feedback);
            for fb in feedback {
                match &smithay_output {
                    Some(o) => fb.presented(
                        o,
                        Duration::from_nanos(time.0),
                        refresh_kind,
                        seq,
                        Kind::from_bits_truncate(flags.bits()),
                    ),
                    None => fb.discarded(),
                }
            }
        }
        self.emit(Event::Presented {
            surface,
            commit,
            output,
            seq,
            time,
            refresh,
            flags: flags.bits(),
        });
        self.frames.collect(commit);
    }

    fn fire_callbacks(
        &mut self,
        time: Timestamp,
        callbacks: &[FrameCallback],
        sent: &mut Vec<SurfaceId>,
    ) {
        let ms = time.wayland_millis();
        for cb in callbacks {
            let Some(p) = self.frames.pending.get_mut(&cb.commit) else {
                continue;
            };
            let list = std::mem::take(&mut p.callbacks);
            if !list.is_empty() {
                for c in list {
                    c.done(ms);
                }
                sent.push(cb.surface);
            }
            self.frames.collect(cb.commit);
        }
    }

    /// Opens the latch barrier of a commit and lets smithay apply the held transactions.
    fn open_latch(&mut self, commit: CommitId) {
        let Some(client) = self.frames.release_latch(commit) else {
            return;
        };
        if let Some(data) = client.get_data::<ClientState>() {
            let dh = self.dh.clone();
            data.compositor.blocker_cleared(self, &dh);
        }
    }

    /// Drops the held buffer of a commit. A buffer that smithay has not applied yet is never held.
    fn release_buffer(&mut self, surface: SurfaceId, commit: CommitId) {
        if self.frames.held.remove(&commit).is_some() {
            return;
        }
        let waiting = self
            .surfaces
            .get(surface)
            .is_some_and(|r| r.applies.iter().any(|e| e.commit == commit && e.new_buffer));
        if waiting {
            self.frames.released_early.insert(commit);
        }
    }
}
