//! Compositor tracks: vblanks and other deadline wakes per output, session events, configures, scenarios, probes,
//! compositor overhead, captures and user marks.

use std::collections::BTreeMap;

use fakeland_types::{OutputId, ProbeId, RunId, Timestamp};

use crate::event::{AppMarkKind, Event, Record};
use crate::perfetto::chrome::ChromeEvent;
use crate::perfetto::emitter::{COMPOSITOR_PID, Emitter, MIN_DUR};

/// Emits the compositor tracks.
pub(crate) fn emit(em: &mut Emitter<'_>) {
    let mut runs: BTreeMap<RunId, (Timestamp, String)> = BTreeMap::new();
    let mut probes: BTreeMap<ProbeId, (Timestamp, String)> = BTreeMap::new();
    let trace = em.trace;
    for r in &trace.records {
        match &r.event {
            Event::Vblank {
                output,
                seq,
                ideal,
                woke,
                dropped,
            } => {
                let tid = output_thread(em, *output);
                let name = if *dropped { "dropped vblank" } else { "vblank" };
                let ts = em.us(*ideal);
                em.push(
                    ChromeEvent::instant(name, "vblank", ts, COMPOSITOR_PID, tid)
                        .arg("seq", *seq)
                        .arg("late_us", (*woke - *ideal).as_micros_f64()),
                );
            }
            Event::DeadlineWake {
                output,
                deadline,
                woke,
            } => {
                let tid = output_thread(em, *output);
                let ts = em.us(*deadline);
                em.push(
                    ChromeEvent::instant("deadline", "vblank", ts, COMPOSITOR_PID, tid)
                        .arg("late_us", (*woke - *deadline).as_micros_f64()),
                );
            }
            Event::Configure {
                surface,
                serial,
                width,
                height,
                states,
            } => {
                let pid = em.surface_pid(*surface);
                let tid = em
                    .tracks
                    .thread(pid, &format!("{} configure", em.surface_label(*surface)));
                let ts = em.us(r.t);
                em.push(
                    ChromeEvent::instant(format!("configure {serial}"), "configure", ts, pid, tid)
                        .arg("size", format!("{width}x{height}"))
                        .arg("states", states.join(" ")),
                );
            }
            Event::AckConfigure { surface, serial } => {
                let pid = em.surface_pid(*surface);
                let tid = em
                    .tracks
                    .thread(pid, &format!("{} configure", em.surface_label(*surface)));
                let ts = em.us(r.t);
                em.push(ChromeEvent::instant(
                    format!("ack {serial}"),
                    "configure",
                    ts,
                    pid,
                    tid,
                ));
            }
            Event::ScenarioStarted { run, name } => {
                runs.insert(*run, (r.t, name.clone()));
            }
            Event::ScenarioStep { run, index, label } => {
                let tid = em.tracks.thread(COMPOSITOR_PID, "scenario steps");
                let ts = em.us(r.t);
                em.push(
                    ChromeEvent::instant(label.clone(), "scenario", ts, COMPOSITOR_PID, tid)
                        .arg("run", run.0)
                        .arg("index", *index),
                );
            }
            Event::ScenarioFinished { run, ok, error } => {
                let (start, name) = runs.remove(run).unwrap_or_else(|| (r.t, run.to_string()));
                span(em, "scenarios", &name, start, r.t, |e| {
                    e.arg("run", run.0)
                        .arg("ok", *ok)
                        .opt_arg("error", error.clone())
                });
            }
            Event::ProbeStarted { probe, kind } => {
                probes.insert(*probe, (r.t, kind.clone()));
            }
            Event::ProbeInput { probe, input } => {
                let tid = em.tracks.thread(COMPOSITOR_PID, "probe events");
                let ts = em.us(r.t);
                em.push(
                    ChromeEvent::instant(
                        format!("{probe} input"),
                        "probe",
                        ts,
                        COMPOSITOR_PID,
                        tid,
                    )
                    .arg("input", input.0),
                );
            }
            Event::ProbeDetected {
                probe,
                commit,
                detector,
            } => {
                let (start, kind) = probes
                    .remove(probe)
                    .unwrap_or_else(|| (r.t, probe.to_string()));
                span(em, "probes", &kind, start, r.t, |e| {
                    e.arg("probe", probe.0)
                        .arg("commit", commit.0)
                        .arg("detector", detector.clone())
                });
            }
            Event::ProbeTimedOut { probe } => {
                let (start, kind) = probes
                    .remove(probe)
                    .unwrap_or_else(|| (r.t, probe.to_string()));
                span(
                    em,
                    "probes",
                    &format!("{kind} timed out"),
                    start,
                    r.t,
                    |e| e.arg("probe", probe.0),
                );
            }
            Event::CompositorOverhead { what, duration } => {
                span(em, "overhead", what, r.t - *duration, r.t, |e| e);
            }
            Event::Capture { what, cost } => {
                span(em, "capture", what, r.t - *cost, r.t, |e| e);
            }
            Event::UserMark { label } => {
                let ts = em.us(r.t);
                em.push(ChromeEvent::global_instant(
                    label.clone(),
                    ts,
                    COMPOSITOR_PID,
                ));
            }
            Event::AppMark {
                surface: None,
                commit: None,
                mark: AppMarkKind::Mark { name },
            } => {
                let ts = em.us(r.t);
                em.push(ChromeEvent::global_instant(
                    name.clone(),
                    ts,
                    COMPOSITOR_PID,
                ));
            }
            _ => session(em, r),
        }
    }
    let end = em.trace.end();
    for (run, (start, name)) in runs {
        span(em, "scenarios", &name, start, end, |e| e.arg("run", run.0));
    }
    for (probe, (start, kind)) in probes {
        span(em, "probes", &kind, start, end, |e| e.arg("probe", probe.0));
    }
}

/// Emits the session events that have no track of their own as instants on the session track.
fn session(em: &mut Emitter<'_>, r: &Record) {
    let name = match &r.event {
        Event::SessionStarted { .. } => "session started".to_string(),
        Event::OutputConfigured(o) | Event::OutputChanged(o) => format!(
            "{} {}x{} @ {} mHz scale {}",
            o.name, o.width, o.height, o.refresh_mhz, o.scale
        ),
        Event::OutputRemoved { output } => format!("{output} removed"),
        Event::OutputPower { output, on } => format!("{output} {}", if *on { "on" } else { "off" }),
        Event::ClientConnected { client, .. } => format!("{client} connected"),
        Event::ClientDisconnected { client } => format!("{client} disconnected"),
        Event::ProcessSpawned { pid, command } => format!("spawn {pid}: {command}"),
        Event::ProcessExited { pid, code, signal } => match (code, signal) {
            (Some(c), _) => format!("{pid} exited {c}"),
            (None, Some(s)) => format!("{pid} killed by signal {s}"),
            (None, None) => format!("{pid} exited"),
        },
        Event::WindowMapped { window, app_id, .. } => format!("{window} mapped {app_id}"),
        Event::WindowUnmapped { window } => format!("{window} unmapped"),
        Event::WindowTitle { window, title } => format!("{window} title {title}"),
        Event::Focus { surface } => match surface {
            Some(s) => format!("focus {s}"),
            None => "focus none".to_string(),
        },
        Event::Clipboard { mime, primary } => format!(
            "{} {}",
            if *primary { "primary" } else { "clipboard" },
            mime.join(" ")
        ),
        _ => return,
    };
    let tid = em.tracks.thread(COMPOSITOR_PID, "session");
    let ts = em.us(r.t);
    em.push(ChromeEvent::instant(
        name,
        "session",
        ts,
        COMPOSITOR_PID,
        tid,
    ));
}

fn output_thread(em: &mut Emitter<'_>, output: OutputId) -> u64 {
    let name = em
        .tl
        .output(output)
        .map_or_else(|| output.to_string(), |o| o.name.clone());
    em.tracks.thread(COMPOSITOR_PID, &format!("{name} vblank"))
}

fn span(
    em: &mut Emitter<'_>,
    track: &str,
    name: &str,
    from: Timestamp,
    to: Timestamp,
    args: impl FnOnce(ChromeEvent) -> ChromeEvent,
) {
    let start = em.us(from).max(0.0);
    let end = em.us(to).max(start + MIN_DUR);
    let tid = em.tracks.lane(COMPOSITOR_PID, track, start, end);
    let e = ChromeEvent::complete(
        name,
        track_cat(track),
        start,
        end - start,
        COMPOSITOR_PID,
        tid,
    );
    em.push(args(e));
}

fn track_cat(track: &str) -> &'static str {
    match track {
        "scenarios" => "scenario",
        "probes" => "probe",
        "capture" => "capture",
        _ => "overhead",
    }
}
