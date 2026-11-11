//! Subsurfaces: separate frame lists, the window tree, and input attribution in the tree.

use fakeland_types::{CommitId, Nanos, SurfaceId, WindowId};

use crate::event::{InputKind, SyncKind};
use crate::frames::{Attribution, FrameTimeline};
use crate::testing::{Fixture, vblank_time};

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    f.subsurface(vblank_time(0), 2, 1);
    f.subsurface(vblank_time(0), 3, 2);
    f.commit(
        vblank_time(0) + Nanos::from_millis(1),
        2,
        10,
        SyncKind::None,
    );
    f.commit(
        vblank_time(0) + Nanos::from_millis(2),
        1,
        11,
        SyncKind::None,
    );
    f.present(2, 10, 1);
    f.present(1, 11, 1);
    f.input(
        vblank_time(1) + Nanos::from_millis(1),
        1,
        3,
        InputKind::PointerMotion,
    );
    f.empty_commit(vblank_time(1) + Nanos::from_millis(2), 3, 12);
    f.commit(
        vblank_time(1) + Nanos::from_millis(5),
        1,
        13,
        SyncKind::None,
    );
    f.present(1, 13, 2);
    f
}

#[test]
fn subsurfaces_have_their_own_frames() {
    let tl = FrameTimeline::build(&fixture().build());
    assert_eq!(tl.frames(SurfaceId(1)).len(), 2);
    assert_eq!(tl.frames(SurfaceId(2)).len(), 1);
    assert_eq!(tl.frames(SurfaceId(3)).len(), 1);
    assert_eq!(tl.children(SurfaceId(1)), vec![SurfaceId(2)]);
    assert_eq!(tl.window_surfaces(WindowId(1)).len(), 3);
    assert_eq!(tl.window_of(SurfaceId(3)), Some(WindowId(1)));
    assert_eq!(
        tl.surface(SurfaceId(3)).and_then(|s| s.parent),
        Some(SurfaceId(2))
    );
    assert_eq!(tl.committed_surfaces().count(), 3);
}

#[test]
fn input_to_a_subsurface_maps_to_the_next_buffer_in_the_tree() {
    let tl = FrameTimeline::build(&fixture().build());
    let input = &tl.inputs()[0];
    // The subsurface commit carries no buffer, so the toplevel commit handles the input.
    assert_eq!(input.handled_by, Some(CommitId(13)));
    assert_eq!(input.attribution, Attribution::NextCommit);
    assert_eq!(input.presented, Some(vblank_time(2)));
    assert_eq!(
        input.latency(),
        Some(vblank_time(2) - (vblank_time(1) + Nanos::from_millis(1)))
    );
    assert_eq!(input.handling_latency(), Some(Nanos::from_millis(4)));
    // The input lists on the frame of the surface that receives it.
    assert_eq!(tl.frames(SurfaceId(3))[0].inputs.len(), 1);
    assert!(tl.frames(SurfaceId(1))[1].inputs.is_empty());
}

#[test]
fn window_filter_builds_the_same_frames() {
    let trace = fixture().build();
    let filtered = trace.for_window(WindowId(1));
    let a = FrameTimeline::build(&trace);
    let b = FrameTimeline::build(&filtered);
    assert_eq!(a.all_frames().count(), b.all_frames().count());
}
