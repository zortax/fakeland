//! Counter tracks: frame rate and damage per surface, process and GPU samples, the cumulative
//! meters, and compositor wake lateness per output.

use fakeland_types::SurfaceId;

use crate::event::Event;
use crate::perfetto::chrome::ChromeEvent;
use crate::perfetto::emitter::{COMPOSITOR_PID, Emitter};
use crate::perfetto::meters;

const MIB: f64 = 1024.0 * 1024.0;

/// Emits every counter sample.
pub(crate) fn emit(em: &mut Emitter<'_>) {
    surfaces(em);
    samples(em);
    meters::emit(em);
    lateness(em);
}

fn surfaces(em: &mut Emitter<'_>) {
    let tl = em.tl;
    let surfaces: Vec<SurfaceId> = tl.committed_surfaces().collect();
    for surface in surfaces {
        let pid = em.surface_pid(surface);
        for p in tl.presented_frames(surface, None) {
            if let Some(interval) = p.interval.filter(|n| n.as_nanos() > 0) {
                let ts = em.us(p.time);
                em.push(ChromeEvent::counter(
                    format!("{surface} fps"),
                    ts,
                    pid,
                    1.0 / interval.as_secs_f64(),
                ));
            }
        }
        for f in tl.frames(surface).iter().filter(|f| f.has_buffer()) {
            let ts = em.us(f.committed);
            em.push(ChromeEvent::counter(
                format!("{surface} damage px"),
                ts,
                pid,
                f.damage_area as f64,
            ));
        }
    }
}

fn samples(em: &mut Emitter<'_>) {
    let trace = em.trace;
    for r in &trace.records {
        let ts = em.us(r.t);
        match &r.event {
            Event::ProcessSample {
                pid,
                cpu_percent,
                rss_bytes,
                threads,
            } => {
                let p = em.os_pid(*pid);
                em.push(ChromeEvent::counter("cpu %", ts, p, *cpu_percent));
                em.push(ChromeEvent::counter(
                    "rss MiB",
                    ts,
                    p,
                    *rss_bytes as f64 / MIB,
                ));
                em.push(ChromeEvent::counter("threads", ts, p, f64::from(*threads)));
            }
            Event::GpuSample {
                util_percent,
                mem_used_bytes,
                ..
            } => {
                em.push(ChromeEvent::counter(
                    "gpu %",
                    ts,
                    COMPOSITOR_PID,
                    *util_percent,
                ));
                em.push(ChromeEvent::counter(
                    "gpu mem MiB",
                    ts,
                    COMPOSITOR_PID,
                    *mem_used_bytes as f64 / MIB,
                ));
            }
            _ => {}
        }
    }
}

fn lateness(em: &mut Emitter<'_>) {
    let tl = em.tl;
    for output in tl.vblank_outputs() {
        let name = tl
            .output(output)
            .map_or_else(|| output.to_string(), |o| o.name.clone());
        for v in tl.vblanks(output) {
            let ts = em.us(v.woke);
            em.push(ChromeEvent::counter(
                format!("{name} wake lateness us"),
                ts,
                COMPOSITOR_PID,
                v.lateness().as_micros_f64(),
            ));
        }
    }
}
