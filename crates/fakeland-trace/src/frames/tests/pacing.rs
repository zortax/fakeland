//! Pacing cases: discarded mailbox frames, missed vblanks and pending frames.

use fakeland_types::{CommitId, Nanos, SurfaceId};

use crate::event::SyncKind;
use crate::frames::{FrameOutcome, FrameTimeline};
use crate::testing::{Fixture, vblank_time};

#[test]
fn mailbox_discards_the_older_commit() {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    f.vblank(0, Nanos::ZERO);
    // Two commits in one refresh cycle. The second replaces the first at the latch.
    f.commit(vblank_time(0) + Nanos::from_millis(2), 1, 1, SyncKind::None);
    f.commit(vblank_time(0) + Nanos::from_millis(8), 1, 2, SyncKind::None);
    f.discard(vblank_time(0) + Nanos::from_millis(8), 1, 1);
    f.release(vblank_time(0) + Nanos::from_millis(8), 1, 1);
    f.latch(vblank_time(1) - Nanos::from_millis(1), 1, 2, 1);
    f.present(1, 2, 1);
    f.commit(vblank_time(1) + Nanos::from_millis(2), 1, 3, SyncKind::None);
    f.latch(vblank_time(2) - Nanos::from_millis(1), 1, 3, 2);
    f.present(1, 3, 2);
    let tl = FrameTimeline::build(&f.build());
    let frames = tl.frames(SurfaceId(1));
    assert_eq!(frames[0].outcome(), FrameOutcome::Discarded);
    assert_eq!(
        frames[0].discarded,
        Some(vblank_time(0) + Nanos::from_millis(8))
    );
    assert_eq!(frames[0].display_latency, None);
    assert_eq!(frames[0].latched, None);
    assert_eq!(frames[1].outcome(), FrameOutcome::Presented);
    assert_eq!(frames[1].present_interval, None);
    assert_eq!(
        frames[2].present_interval,
        Some(vblank_time(2) - vblank_time(1))
    );
    // Without a callback the client time is unknown.
    assert_eq!(frames[1].client_time, None);
}

#[test]
fn missed_vblanks_count_between_presentations() {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    for (c, seq) in [(1, 1), (2, 2), (3, 5), (4, 6)] {
        f.commit(
            vblank_time(seq - 1) + Nanos::from_millis(3),
            1,
            c,
            SyncKind::None,
        );
        f.present(1, c, seq);
    }
    let tl = FrameTimeline::build(&f.build());
    let missed: Vec<_> = tl
        .frames(SurfaceId(1))
        .iter()
        .map(|f| f.vblanks_missed)
        .collect();
    assert_eq!(missed, [None, Some(0), Some(2), Some(0)]);
    let intervals: Vec<_> = tl
        .presented_frames(SurfaceId(1), None)
        .iter()
        .map(|p| p.interval)
        .collect();
    assert_eq!(intervals[2], Some(vblank_time(5) - vblank_time(2)));
}

#[test]
fn a_frame_without_an_end_is_pending() {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    f.commit(vblank_time(0), 1, 1, SyncKind::Explicit);
    let tl = FrameTimeline::build(&f.build());
    let frame = tl.frame(CommitId(1)).unwrap();
    assert_eq!(frame.outcome(), FrameOutcome::Pending);
    assert_eq!(frame.gpu_time, None);
    assert_eq!(frame.queue_time, None);
}

#[test]
fn callbacks_pair_with_the_next_commit_only() {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    f.callback(vblank_time(0), 1);
    f.callback(vblank_time(1), 1);
    f.commit(vblank_time(1) + Nanos::from_millis(4), 1, 1, SyncKind::None);
    f.commit(vblank_time(1) + Nanos::from_millis(6), 1, 2, SyncKind::None);
    let tl = FrameTimeline::build(&f.build());
    let frames = tl.frames(SurfaceId(1));
    assert_eq!(frames[0].callback_sent, Some(vblank_time(0)));
    assert_eq!(frames[1].callback_sent, None);
}
