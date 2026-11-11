//! The spans in which a toplevel carried the `suspended` state.

use fakeland_trace::{Event, Trace};
use fakeland_types::{SurfaceId, Timestamp};

/// Returns the spans between a configure with `suspended` and the next configure without it. An
/// open span ends at `end`.
pub fn suspended_spans(
    trace: &Trace,
    surface: SurfaceId,
    end: Timestamp,
) -> Vec<(Timestamp, Timestamp)> {
    let mut spans = Vec::new();
    let mut since: Option<Timestamp> = None;
    for r in &trace.records {
        let Event::Configure {
            surface: s, states, ..
        } = &r.event
        else {
            continue;
        };
        if *s != surface {
            continue;
        }
        let suspended = states.iter().any(|x| x == "suspended");
        match (since, suspended) {
            (None, true) => since = Some(r.t),
            (Some(a), false) => {
                spans.push((a, r.t));
                since = None;
            }
            _ => {}
        }
    }
    if let Some(a) = since {
        spans.push((a, end.max(a)));
    }
    spans
}
