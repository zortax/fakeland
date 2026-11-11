//! The scroll amounts of injected axis events and the commit that consumed each one.

use fakeland_trace::frames::Attribution;
use fakeland_trace::{FrameTimeline, InputKind};
use fakeland_types::{CommitId, SurfaceId, Timestamp};
use fakeland_vision::Axis;
use serde::{Deserialize, Serialize};

use crate::analyzers::util::parse_axis_detail;

/// One injected axis event along the scroll axis.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ScrollInput {
    /// The send time.
    pub sent: Timestamp,
    /// The amount in logical pixels; positive scrolls down or right.
    pub amount: f64,
    /// The commit that consumed the event: the commit that marked it handled, else the first
    /// commit of the surface at or after the send time.
    pub commit: Option<CommitId>,
    /// The commit time of that commit.
    pub committed: Option<Timestamp>,
}

/// Returns the axis events of the trace with the commits of `surface` that consumed them.
pub fn scroll_inputs(
    tl: &FrameTimeline,
    surface: Option<SurfaceId>,
    axis: Axis,
) -> Vec<ScrollInput> {
    let frames = surface.map(|s| tl.frames(s)).unwrap_or_default();
    tl.inputs()
        .iter()
        .filter(|i| i.kind == InputKind::PointerAxis)
        .filter_map(|i| {
            let d = parse_axis_detail(&i.detail)?;
            let amount = match axis {
                Axis::Vertical => d.dy,
                Axis::Horizontal => d.dx,
            };
            let marked = (i.attribution == Attribution::Mark)
                .then_some(i.handled_by)
                .flatten()
                .and_then(|c| tl.frame(c));
            let by_time = || {
                frames
                    .iter()
                    .find(|f| f.committed >= i.sent && f.buffer.is_some())
            };
            let consumer = marked.or_else(by_time);
            Some(ScrollInput {
                sent: i.sent,
                amount,
                commit: consumer.map(|f| f.commit),
                committed: consumer.map(|f| f.committed),
            })
        })
        .filter(|s| s.amount != 0.0)
        .collect()
}

/// Returns the sum of the amounts consumed by commits at or before `t`.
pub fn consumed_by(inputs: &[ScrollInput], t: Timestamp) -> f64 {
    inputs
        .iter()
        .filter(|i| i.committed.is_some_and(|c| c <= t))
        .map(|i| i.amount)
        .sum()
}
