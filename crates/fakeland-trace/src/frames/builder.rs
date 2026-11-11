//! The timeline builder: one pass over the records that collects frames and entities.

use std::collections::HashMap;

use fakeland_types::{CommitId, InputEventId, Physical, Rect, SurfaceId, Timestamp};

use crate::event::{AppMarkKind, Event, Record};
use crate::frames::shown::Unshown;
use crate::frames::{
    Attribution, ClientInfo, FrameMarks, FrameRecord, FrameTimeline, InputRecord, LatchInfo,
    PresentInfo, ProcessInfo, SpanMark, SurfaceInfo, VblankInfo, WindowInfo, union_area,
};
use crate::trace::Trace;

/// Collects frames and entities from records in order.
#[derive(Default)]
pub(crate) struct Builder {
    tl: FrameTimeline,
    callbacks: HashMap<SurfaceId, Timestamp>,
    inputs: HashMap<SurfaceId, Vec<InputEventId>>,
    marks_by_surface: HashMap<SurfaceId, Vec<AppMarkKind>>,
    marks_by_commit: HashMap<CommitId, Vec<AppMarkKind>>,
    buffer_size: HashMap<SurfaceId, (i32, i32)>,
    unshown: Unshown,
}

impl Builder {
    /// Reads a whole trace and returns the raw timeline, before derivation and attribution.
    pub(crate) fn run(trace: &Trace) -> FrameTimeline {
        let mut b = Self::default();
        for o in &trace.header.outputs {
            b.tl.outputs.insert(o.output, o.clone());
        }
        for r in &trace.records {
            b.observe(r);
        }
        b.tl
    }

    fn observe(&mut self, r: &Record) {
        let t = r.t;
        self.tl.tree.observe(&r.event);
        match &r.event {
            Event::OutputConfigured(info) | Event::OutputChanged(info) => {
                self.tl.outputs.insert(info.output, info.clone());
            }
            Event::Vblank {
                output,
                seq,
                ideal,
                woke,
                dropped,
            } => self
                .tl
                .vblanks
                .entry(*output)
                .or_default()
                .push(VblankInfo {
                    output: *output,
                    seq: *seq,
                    ideal: *ideal,
                    woke: *woke,
                    dropped: *dropped,
                }),
            Event::ClientConnected { client, pid } => {
                self.tl.clients.insert(
                    *client,
                    ClientInfo {
                        client: *client,
                        pid: *pid,
                        connected: Some(t),
                        disconnected: None,
                    },
                );
            }
            Event::ClientDisconnected { client } => {
                self.tl
                    .clients
                    .entry(*client)
                    .or_insert_with(|| ClientInfo {
                        client: *client,
                        pid: None,
                        connected: None,
                        disconnected: None,
                    })
                    .disconnected = Some(t);
            }
            Event::ProcessSpawned { pid, command } => {
                self.tl.processes.insert(
                    *pid,
                    ProcessInfo {
                        pid: *pid,
                        command: command.clone(),
                        spawned: t,
                        exited: None,
                        code: None,
                        signal: None,
                    },
                );
            }
            Event::ProcessExited { pid, code, signal } => {
                if let Some(p) = self.tl.processes.get_mut(pid) {
                    p.exited = Some(t);
                    p.code = *code;
                    p.signal = *signal;
                }
            }
            Event::SurfaceCreated { surface, client } => {
                let s = self.surface(*surface);
                s.client = Some(*client);
                s.created = Some(t);
            }
            Event::SurfaceDestroyed { surface } => self.surface(*surface).destroyed = Some(t),
            Event::SurfaceRole {
                surface,
                role,
                parent,
            } => {
                let s = self.surface(*surface);
                s.role = Some(role.clone());
                s.parent = *parent;
            }
            Event::WindowMapped {
                window,
                surface,
                app_id,
                title,
                output,
            } => {
                self.tl.windows.insert(
                    *window,
                    WindowInfo {
                        window: *window,
                        surface: *surface,
                        app_id: app_id.clone(),
                        title: title.clone(),
                        output: *output,
                        mapped: t,
                        unmapped: None,
                    },
                );
            }
            Event::WindowUnmapped { window } => {
                if let Some(w) = self.tl.windows.get_mut(window) {
                    w.unmapped = Some(t);
                }
            }
            Event::WindowTitle { window, title } => {
                if let Some(w) = self.tl.windows.get_mut(window) {
                    w.title.clone_from(title);
                }
            }
            Event::WindowAppId { window, app_id } => {
                if let Some(w) = self.tl.windows.get_mut(window) {
                    w.app_id.clone_from(app_id);
                }
            }
            Event::FrameCallbackSent { surface, .. } => {
                self.callbacks.entry(*surface).or_insert(t);
            }
            Event::InputSent {
                id,
                kind,
                surface,
                x,
                y,
                detail,
                source,
            } => {
                if let Some(s) = surface {
                    self.inputs.entry(*s).or_default().push(*id);
                }
                self.tl.input_index.insert(*id, self.tl.inputs.len());
                self.tl.inputs.push(InputRecord {
                    id: *id,
                    sent: t,
                    kind: *kind,
                    surface: *surface,
                    x: *x,
                    y: *y,
                    detail: detail.clone(),
                    source: *source,
                    handled_by: None,
                    attribution: Attribution::None,
                    handled_at: None,
                    presented_commit: None,
                    presented: None,
                });
            }
            Event::AppMark {
                surface,
                commit,
                mark,
            } => self.mark(*surface, *commit, mark),
            Event::Commit { .. } => self.commit(t, &r.event),
            Event::Ready { commit, .. } => {
                if let Some(f) = self.tl.frame_mut(*commit) {
                    f.ready.get_or_insert(t);
                } else {
                    self.tl.orphans += 1;
                }
            }
            Event::Latched {
                commit,
                output,
                seq,
                ..
            } => {
                if let Some(f) = self.tl.frame_mut(*commit) {
                    f.latched.get_or_insert(LatchInfo {
                        t,
                        output: *output,
                        seq: *seq,
                    });
                } else {
                    self.tl.orphans += 1;
                }
            }
            Event::Presented {
                surface,
                commit,
                output,
                seq,
                time,
                refresh,
                flags,
            } => {
                let info = PresentInfo {
                    t,
                    time: *time,
                    output: *output,
                    seq: *seq,
                    refresh: *refresh,
                    flags: *flags,
                    via: None,
                };
                self.presented(*surface, *commit, info);
            }
            Event::Discarded { surface, commit } => {
                if let Some(f) = self.tl.frame_mut(*commit) {
                    f.discarded.get_or_insert(t);
                    let content = f.has_content();
                    self.unshown.discarded(*surface, *commit, content);
                } else {
                    self.tl.orphans += 1;
                }
            }
            Event::Released { commit, .. } => {
                if let Some(f) = self.tl.frame_mut(*commit) {
                    f.released.get_or_insert(t);
                } else {
                    self.tl.orphans += 1;
                }
            }
            _ => {}
        }
    }

    /// Records a presentation. A presented commit without content passes the presentation to
    /// the discarded content commit whose buffer the refresh shows.
    fn presented(&mut self, surface: SurfaceId, commit: CommitId, info: PresentInfo) {
        let (content, latched) = match self.tl.frame_mut(commit) {
            Some(f) => {
                f.presented.get_or_insert(info);
                (Some(f.has_content()), f.latched)
            }
            None => {
                self.tl.orphans += 1;
                (None, None)
            }
        };
        let Some(shown) = self.unshown.presented(surface, content) else {
            return;
        };
        if let Some(f) = self.tl.frame_mut(shown) {
            f.presented.get_or_insert(PresentInfo {
                via: Some(commit),
                ..info
            });
            if f.latched.is_none() {
                f.latched = latched;
            }
        }
    }

    fn surface(&mut self, surface: SurfaceId) -> &mut SurfaceInfo {
        self.tl
            .surfaces
            .entry(surface)
            .or_insert_with(|| SurfaceInfo::unknown(surface))
    }

    fn mark(&mut self, surface: Option<SurfaceId>, commit: Option<CommitId>, mark: &AppMarkKind) {
        match (commit, surface) {
            (Some(c), _) => match self.tl.frame_mut(c) {
                Some(f) => apply_mark(&mut f.marks, mark),
                None => self
                    .marks_by_commit
                    .entry(c)
                    .or_default()
                    .push(mark.clone()),
            },
            (None, Some(s)) => self
                .marks_by_surface
                .entry(s)
                .or_default()
                .push(mark.clone()),
            (None, None) => self.tl.global_marks += 1,
        }
    }

    fn commit(&mut self, t: Timestamp, event: &Event) {
        let Event::Commit {
            surface,
            commit,
            buffer,
            damage,
            frame_callback,
            presentation_feedback,
            sync,
            fifo_barrier,
            fifo_wait,
            target,
            configure_serial,
            detached,
        } = event
        else {
            return;
        };
        if self.tl.index.contains_key(commit) {
            return;
        }
        self.surface(*surface);
        if let Some(b) = buffer {
            self.buffer_size.insert(*surface, (b.width, b.height));
        }
        let bounds = self
            .buffer_size
            .get(surface)
            .map(|(w, h)| Rect::<Physical>::new(0, 0, *w, *h));
        let mut marks = FrameMarks::default();
        for m in self
            .marks_by_surface
            .remove(surface)
            .into_iter()
            .chain(self.marks_by_commit.remove(commit))
            .flatten()
        {
            apply_mark(&mut marks, &m);
        }
        let frame = FrameRecord {
            commit: *commit,
            surface: *surface,
            callback_sent: self.callbacks.remove(surface),
            committed: t,
            ready: None,
            latched: None,
            presented: None,
            discarded: None,
            released: None,
            buffer: buffer.clone(),
            detached: *detached,
            damage: damage.clone(),
            damage_area: union_area(damage, bounds),
            configure_serial: *configure_serial,
            sync: *sync,
            frame_callback: *frame_callback,
            presentation_feedback: *presentation_feedback,
            fifo_barrier: *fifo_barrier,
            fifo_wait: *fifo_wait,
            target: *target,
            marks,
            inputs: self.inputs.remove(surface).unwrap_or_default(),
            client_time: None,
            gpu_time: None,
            queue_time: None,
            display_latency: None,
            present_interval: None,
            vblanks_missed: None,
        };
        let frames = self.tl.frames.entry(*surface).or_default();
        self.tl.index.insert(*commit, (*surface, frames.len()));
        frames.push(frame);
    }
}

fn apply_mark(marks: &mut FrameMarks, mark: &AppMarkKind) {
    match mark {
        AppMarkKind::Frame { frame_id } => marks.frame_id = Some(*frame_id),
        AppMarkKind::InputHandled { input } => {
            if !marks.inputs_handled.contains(input) {
                marks.inputs_handled.push(*input);
            }
        }
        AppMarkKind::Span { name, begin, end } => marks.spans.push(SpanMark {
            name: name.clone(),
            begin: *begin,
            end: *end,
        }),
        AppMarkKind::Mark { name } => marks.names.push(name.clone()),
    }
}
