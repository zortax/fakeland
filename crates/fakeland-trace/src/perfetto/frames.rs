//! Frame slices per surface: the whole frame, each stage, the time on screen, app spans and marks.

use fakeland_types::{Nanos, SurfaceId};

use crate::event::SyncKind;
use crate::frames::FrameRecord;
use crate::perfetto::chrome::ChromeEvent;
use crate::perfetto::emitter::{Emitter, MIN_DUR, Slot};

/// Emits the frame slices of every surface.
pub(crate) fn emit(em: &mut Emitter<'_>) {
    let surfaces: Vec<SurfaceId> = em.tl.committed_surfaces().collect();
    for surface in surfaces {
        let pid = em.surface_pid(surface);
        let label = em.surface_label(surface);
        name_tracks(em, pid, &label, surface);
        let tl = em.tl;
        for f in tl.frames(surface) {
            frame(em, pid, &label, f);
            stages(em, pid, &label, f);
            marks(em, pid, &label, f);
        }
        on_screen(em, pid, &label, surface);
    }
}

/// Creates the surface threads in a fixed order, only for stages that occur.
fn name_tracks(em: &mut Emitter<'_>, pid: u64, label: &str, surface: SurfaceId) {
    let frames = em.tl.frames(surface);
    let any = |p: fn(&FrameRecord) -> bool| frames.iter().any(p);
    let single_output = em.tl.outputs().count() <= 1;
    let order: [(&str, bool); 8] = [
        ("frames", true),
        ("client", any(|f| f.client_time.is_some())),
        ("gpu", any(|f| f.gpu_time.is_some())),
        ("queue", any(|f| f.queue_time.is_some())),
        (
            "display",
            any(|f| f.latched.is_some() && f.presented.is_some()),
        ),
        ("on screen", single_output && any(|f| f.presented.is_some())),
        ("app spans", any(|f| !f.marks.spans.is_empty())),
        ("marks", any(|f| !f.marks.names.is_empty())),
    ];
    for (name, used) in order {
        if used {
            em.tracks.thread(pid, &format!("{label} {name}"));
        }
    }
}

fn ms(n: Option<Nanos>) -> Option<f64> {
    n.map(Nanos::as_millis_f64)
}

fn frame(em: &mut Emitter<'_>, pid: u64, label: &str, f: &FrameRecord) {
    let start = em.us(f.committed);
    let end_t = f
        .presented
        .map(|p| p.time)
        .or(f.discarded)
        .or(f.latched.map(|l| l.t))
        .or(f.ready)
        .unwrap_or(f.committed);
    let end = em.us(end_t).max(start + MIN_DUR);
    let tid = em.tracks.lane(pid, &format!("{label} frames"), start, end);
    em.frame_slots.insert(f.commit, Slot { pid, tid, start });
    let name = match f.marks.frame_id {
        Some(id) => format!("{} frame {id}", f.commit),
        None => f.commit.to_string(),
    };
    let b = f.buffer.as_ref();
    let e = ChromeEvent::complete(name, "frame", start, end - start, pid, tid)
        .arg("commit", f.commit.0)
        .arg("outcome", f.outcome().as_str())
        .arg("sync", f.sync.as_str())
        .arg("damage_area", f.damage_area)
        .arg("inputs", f.inputs.len())
        .opt_arg("seq", f.presented.map(|p| p.seq))
        .opt_arg("buffer", b.map(|b| b.id.0))
        .opt_arg("buffer_kind", b.map(|b| b.kind.as_str()))
        .opt_arg("format", b.map(|b| b.fourcc()))
        .opt_arg("size", b.map(|b| format!("{}x{}", b.width, b.height)))
        .opt_arg("scale", b.map(|b| b.scale))
        .opt_arg("configure_serial", f.configure_serial)
        .opt_arg("app_frame_id", f.marks.frame_id)
        .opt_arg("client_ms", ms(f.client_time))
        .opt_arg("gpu_ms", ms(f.gpu_time))
        .opt_arg("queue_ms", ms(f.queue_time))
        .opt_arg("display_latency_ms", ms(f.display_latency))
        .opt_arg("present_interval_ms", ms(f.present_interval))
        .opt_arg("vblanks_missed", f.vblanks_missed);
    em.push(e);
}

fn stages(em: &mut Emitter<'_>, pid: u64, label: &str, f: &FrameRecord) {
    let stage = |em: &mut Emitter<'_>, name: &str, from, to| {
        let (a, b) = (em.us(from), em.us(to));
        let tid = em.tracks.lane(pid, &format!("{label} {name}"), a, b);
        em.push(ChromeEvent::complete(name, "stage", a, b - a, pid, tid).arg("commit", f.commit.0));
    };
    if let Some(cb) = f.callback_sent {
        stage(em, "client", cb, f.committed);
    }
    if let (Some(ready), true) = (f.ready, f.sync != SyncKind::None) {
        stage(em, "gpu", f.committed, ready);
    }
    if let Some(l) = f.latched {
        stage(em, "queue", f.effective_ready(), l.t);
        if let Some(p) = f.presented {
            stage(em, "display", l.t, p.time);
        }
    }
}

fn marks(em: &mut Emitter<'_>, pid: u64, label: &str, f: &FrameRecord) {
    for s in &f.marks.spans {
        let (a, b) = (em.us(s.begin), em.us(s.end));
        let tid = em.tracks.lane(pid, &format!("{label} app spans"), a, b);
        em.push(
            ChromeEvent::complete(s.name.clone(), "app", a, b - a, pid, tid)
                .arg("commit", f.commit.0),
        );
    }
    if !f.marks.names.is_empty() {
        let tid = em.tracks.thread(pid, &format!("{label} marks"));
        let ts = em.us(f.committed);
        for name in &f.marks.names {
            em.push(
                ChromeEvent::instant(name.clone(), "app", ts, pid, tid).arg("commit", f.commit.0),
            );
        }
    }
}

/// Emits one slice per presented frame from its presentation to the next one on the output.
fn on_screen(em: &mut Emitter<'_>, pid: u64, label: &str, surface: SurfaceId) {
    let tl = em.tl;
    let outputs: Vec<_> = tl.outputs().map(|o| o.output).collect();
    let mut seen = outputs.clone();
    for f in tl.presented(surface) {
        if let Some(p) = f.presented
            && !seen.contains(&p.output)
        {
            seen.push(p.output);
        }
    }
    for output in seen {
        let frames = tl.presented_frames(surface, Some(output));
        let track = match tl.output(output) {
            Some(o) if outputs.len() > 1 => format!("{label} on screen {}", o.name),
            _ => format!("{label} on screen"),
        };
        for (i, p) in frames.iter().enumerate() {
            let start = em.us(p.time);
            let end = frames
                .get(i + 1)
                .map_or_else(|| em.end_us(), |n| em.us(n.time))
                .max(start + MIN_DUR);
            let tid = em.tracks.lane(pid, &track, start, end);
            em.screen_slots.insert(p.commit, Slot { pid, tid, start });
            em.push(
                ChromeEvent::complete(p.commit.to_string(), "screen", start, end - start, pid, tid)
                    .arg("seq", p.seq)
                    .opt_arg("vblanks_missed", p.vblanks_missed),
            );
        }
    }
}
