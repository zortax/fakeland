//! GPU synchronization: explicit and implicit fences against commits without fences.

use fakeland_types::{CommitId, Nanos};

use crate::event::SyncKind;
use crate::frames::FrameTimeline;
use crate::testing::{Fixture, vblank_time};

fn flow(sync: SyncKind, ready: bool) -> FrameTimeline {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    let commit = vblank_time(0) + Nanos::from_millis(4);
    f.commit(commit, 1, 1, sync);
    if ready {
        f.ready(commit + Nanos::from_millis(6), 1, 1);
    }
    f.latch(vblank_time(1) - Nanos::from_millis(1), 1, 1, 1);
    f.present(1, 1, 1);
    FrameTimeline::build(&f.build())
}

#[test]
fn explicit_sync_measures_gpu_time() {
    let tl = flow(SyncKind::Explicit, true);
    let f = tl.frame(CommitId(1)).unwrap();
    assert_eq!(f.gpu_time, Some(Nanos::from_millis(6)));
    let latch = vblank_time(1) - Nanos::from_millis(1);
    let ready = vblank_time(0) + Nanos::from_millis(10);
    assert_eq!(f.queue_time, Some(latch - ready));
}

#[test]
fn implicit_sync_measures_gpu_time() {
    let tl = flow(SyncKind::Implicit, true);
    assert_eq!(
        tl.frame(CommitId(1)).and_then(|f| f.gpu_time),
        Some(Nanos::from_millis(6))
    );
}

#[test]
fn no_sync_has_no_gpu_time() {
    let tl = flow(SyncKind::None, true);
    let f = tl.frame(CommitId(1)).unwrap();
    assert_eq!(f.gpu_time, None);
    let tl = flow(SyncKind::None, false);
    let f = tl.frame(CommitId(1)).unwrap();
    assert_eq!(f.gpu_time, None);
    assert_eq!(f.effective_ready(), f.committed);
    let latch = vblank_time(1) - Nanos::from_millis(1);
    assert_eq!(f.queue_time, Some(latch - f.committed));
}
