//! Commits without a buffer: a refresh that presents one shows no new frame, and a content commit
//! that one replaces at the latch keeps the presentation.

use fakeland_types::{CommitId, Nanos, OutputId, SurfaceId};

use crate::event::SyncKind;
use crate::frames::{FrameOutcome, FrameTimeline};
use crate::summary::SurfaceSummary;
use crate::testing::{Fixture, vblank_time};

const LATCH: Nanos = Nanos::from_millis(1);

#[test]
fn a_presented_commit_without_content_is_no_frame() {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    f.commit(vblank_time(0) + Nanos::from_millis(2), 1, 1, SyncKind::None);
    f.latch(vblank_time(1) - LATCH, 1, 1, 1);
    f.present(1, 1, 1);
    f.empty_commit(vblank_time(1) + Nanos::from_millis(2), 1, 2);
    f.latch(vblank_time(1) + Nanos::from_millis(2), 1, 2, 2);
    f.present(1, 2, 2);
    f.commit(vblank_time(2) + Nanos::from_millis(2), 1, 3, SyncKind::None);
    f.latch(vblank_time(3) - LATCH, 1, 3, 3);
    f.present(1, 3, 3);
    let tl = FrameTimeline::build(&f.build());

    let empty = tl.frame(CommitId(2)).unwrap();
    assert_eq!(empty.outcome(), FrameOutcome::Presented);
    assert!(!empty.is_presented_frame());
    assert_eq!(empty.present_interval, None);
    assert_eq!(empty.vblanks_missed, None);

    let p = tl.presented_frames(SurfaceId(1), None);
    let commits: Vec<_> = p.iter().map(|p| p.commit).collect();
    assert_eq!(commits, [CommitId(1), CommitId(3)]);
    assert_eq!(p[1].vblanks_missed, Some(1));
    assert_eq!(p[1].interval, Some(vblank_time(3) - vblank_time(1)));
    assert_eq!(tl.frame(CommitId(3)).unwrap().vblanks_missed, Some(1));
    assert_eq!(tl.presented(SurfaceId(1)).count(), 2);
    assert_eq!(tl.presented_on(OutputId(0)).len(), 2);

    let s = SurfaceSummary::build(&tl, SurfaceId(1));
    assert_eq!((s.presented, s.discarded, s.pending), (2, 0, 0));
    assert_eq!(s.vblanks_missed, 1);
    assert_eq!(s.frames_after_miss, 1);
}

#[test]
fn content_replaced_by_a_commit_without_content_keeps_the_presentation() {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    f.commit(vblank_time(0) + Nanos::from_millis(2), 1, 1, SyncKind::None);
    f.latch(vblank_time(1) - LATCH, 1, 1, 1);
    f.present(1, 1, 1);
    // A content commit and a commit without content in one refresh: the latch takes both, the
    // scheduler discards the first and presents the second, and the refresh shows the first.
    f.commit(vblank_time(1) + Nanos::from_millis(3), 1, 2, SyncKind::None);
    f.empty_commit(vblank_time(1) + Nanos::from_millis(5), 1, 3);
    f.discard(vblank_time(2) - LATCH, 1, 2);
    f.latch(vblank_time(2) - LATCH, 1, 3, 2);
    f.present(1, 3, 2);
    f.release(vblank_time(2), 1, 1);
    let tl = FrameTimeline::build(&f.build());

    let content = tl.frame(CommitId(2)).unwrap();
    assert_eq!(content.outcome(), FrameOutcome::Presented);
    assert!(content.is_presented_frame());
    assert_eq!(content.discarded, Some(vblank_time(2) - LATCH));
    let p = content.presented.unwrap();
    assert_eq!(
        (p.seq, p.time, p.via),
        (2, vblank_time(2), Some(CommitId(3)))
    );
    assert_eq!(content.latched.map(|l| l.seq), Some(2));
    assert_eq!(
        content.display_latency,
        Some(vblank_time(2) - (vblank_time(1) + Nanos::from_millis(3)))
    );
    assert_eq!(
        content.present_interval,
        Some(vblank_time(2) - vblank_time(1))
    );
    assert_eq!(content.vblanks_missed, Some(0));

    let carrier = tl.frame(CommitId(3)).unwrap();
    assert_eq!(carrier.presented.map(|p| p.via), Some(None));
    assert!(!carrier.is_presented_frame());

    let frames = tl.presented_frames(SurfaceId(1), None);
    let shown: Vec<_> = frames.iter().map(|p| (p.commit, p.seq)).collect();
    assert_eq!(shown, [(CommitId(1), 1), (CommitId(2), 2)]);

    let s = SurfaceSummary::build(&tl, SurfaceId(1));
    assert_eq!((s.presented, s.discarded, s.pending), (2, 0, 0));
    assert_eq!(s.vblanks_missed, 0);
}

#[test]
fn content_replaced_by_newer_content_stays_discarded() {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    f.commit(vblank_time(0) + Nanos::from_millis(2), 1, 1, SyncKind::None);
    f.commit(vblank_time(0) + Nanos::from_millis(4), 1, 2, SyncKind::None);
    f.empty_commit(vblank_time(0) + Nanos::from_millis(6), 1, 3);
    f.discard(vblank_time(1) - LATCH, 1, 1);
    f.discard(vblank_time(1) - LATCH, 1, 2);
    f.latch(vblank_time(1) - LATCH, 1, 3, 1);
    f.present(1, 3, 1);
    let tl = FrameTimeline::build(&f.build());
    assert_eq!(
        tl.frame(CommitId(1)).unwrap().outcome(),
        FrameOutcome::Discarded
    );
    assert_eq!(
        tl.frame(CommitId(2)).unwrap().outcome(),
        FrameOutcome::Presented
    );
    let shown: Vec<_> = tl
        .presented_frames(SurfaceId(1), None)
        .iter()
        .map(|p| p.commit)
        .collect();
    assert_eq!(shown, [CommitId(2)]);
}
