//! Input events: instants on the compositor input track, and flow arrows from each event to the
//! commit that handles it and the presentation that shows it.

use crate::frames::{Attribution, InputRecord};
use crate::perfetto::chrome::ChromeEvent;
use crate::perfetto::emitter::{COMPOSITOR_PID, Emitter, FLOW_EPS, MIN_DUR};

/// Emits every input event. Run it after the frame slices, which the flows bind to.
pub(crate) fn emit(em: &mut Emitter<'_>) {
    let tl = em.tl;
    if tl.inputs().is_empty() {
        return;
    }
    let tid = em.tracks.thread(COMPOSITOR_PID, "input");
    for i in tl.inputs() {
        let ts = em.us(i.sent);
        em.push(
            ChromeEvent::instant(i.kind.as_str(), "input", ts, COMPOSITOR_PID, tid)
                .arg("id", i.id.0)
                .arg("source", i.source.as_str())
                .opt_arg("surface", i.surface.map(|s| s.0))
                .opt_arg("x", i.x)
                .opt_arg("y", i.y)
                .opt_arg("detail", (!i.detail.is_empty()).then(|| i.detail.clone()))
                .opt_arg("handled_by", i.handled_by.map(|c| c.0))
                .opt_arg("latency_ms", i.latency().map(|n| n.as_millis_f64())),
        );
        flow(em, i);
    }
}

/// Emits the handling slice of an input and the flow from it through the handling commit to the
/// presentation.
fn flow(em: &mut Emitter<'_>, i: &InputRecord) {
    let (Some(commit), Some(handled_at)) = (i.handled_by, i.handled_at) else {
        return;
    };
    let Some(frame) = em.frame_slots.get(&commit).copied() else {
        return;
    };
    let start = em.us(i.sent);
    let end = em.us(handled_at).max(start + MIN_DUR);
    let tid = em.tracks.lane(COMPOSITOR_PID, "input handling", start, end);
    let how = match i.attribution {
        Attribution::Mark => "mark",
        Attribution::NextCommit => "next_commit",
        Attribution::None => "none",
    };
    em.push(
        ChromeEvent::complete(
            format!("{} {} -> {commit}", i.id, i.kind.as_str()),
            "input",
            start,
            end - start,
            COMPOSITOR_PID,
            tid,
        )
        .arg("attribution", how),
    );
    let id = i.id.0;
    em.push(ChromeEvent::flow(
        "s",
        id,
        start + FLOW_EPS,
        COMPOSITOR_PID,
        tid,
    ));
    let screen = i
        .presented_commit
        .and_then(|c| em.screen_slots.get(&c).copied());
    match screen {
        Some(screen) => {
            em.push(ChromeEvent::flow(
                "t",
                id,
                frame.start + FLOW_EPS,
                frame.pid,
                frame.tid,
            ));
            em.push(ChromeEvent::flow(
                "f",
                id,
                screen.start + FLOW_EPS,
                screen.pid,
                screen.tid,
            ));
        }
        None => em.push(ChromeEvent::flow(
            "f",
            id,
            frame.start + FLOW_EPS,
            frame.pid,
            frame.tid,
        )),
    }
}
