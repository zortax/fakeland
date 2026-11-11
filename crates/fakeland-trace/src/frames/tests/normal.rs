//! A steady 60 Hz flow: every stage and derived time of each frame.

use fakeland_types::{CommitId, Nanos, OutputId, ProcessId, SurfaceId, WindowId};

use crate::frames::{FrameOutcome, FrameTimeline};
use crate::testing::{PERIOD, steady, vblank_time};

#[test]
fn every_stage_of_a_steady_flow() {
    let tl = FrameTimeline::build(&steady(10).build());
    let frames = tl.frames(SurfaceId(1));
    assert_eq!(frames.len(), 10);
    for (n, f) in frames.iter().enumerate() {
        let n = n as u64;
        assert_eq!(f.commit, CommitId(n + 1));
        assert_eq!(f.outcome(), FrameOutcome::Presented);
        assert_eq!(f.callback_sent, Some(vblank_time(n) + Nanos(30_000)));
        assert_eq!(f.client_time, Some(Nanos(5_000_000 - 30_000)));
        assert_eq!(f.gpu_time, Some(Nanos::from_millis(2)));
        let latch = vblank_time(n + 1) - Nanos::from_micros(1500);
        assert_eq!(
            f.queue_time,
            Some(latch - (vblank_time(n) + Nanos::from_millis(7)))
        );
        assert_eq!(
            f.display_latency,
            Some(vblank_time(n + 1) - (vblank_time(n) + Nanos::from_millis(5)))
        );
        assert_eq!(f.latched.map(|l| l.seq), Some(n + 1));
        assert_eq!(f.presented.map(|p| p.seq), Some(n + 1));
        assert_eq!(f.damage_area, 100 * 100);
        assert_eq!(f.configure_serial, Some(1));
        if n == 0 {
            assert_eq!(f.present_interval, None);
            assert_eq!(f.vblanks_missed, None);
        } else {
            assert_eq!(
                f.present_interval,
                Some(vblank_time(n + 1) - vblank_time(n))
            );
            assert_eq!(f.vblanks_missed, Some(0));
        }
        if n < 8 {
            assert_eq!(f.released, Some(vblank_time(n + 2)));
        } else {
            assert_eq!(f.released, None);
        }
    }
    let interval = frames[5].present_interval.unwrap_or_default();
    assert!((interval - PERIOD).abs() <= Nanos(1));
}

#[test]
fn entities_and_vblanks() {
    let tl = FrameTimeline::build(&steady(3).build());
    let w = tl.window(WindowId(1)).unwrap();
    assert_eq!(w.surface, SurfaceId(1));
    assert_eq!(w.app_id, "org.test.app");
    assert_eq!(tl.window_of(SurfaceId(1)), Some(WindowId(1)));
    assert_eq!(
        tl.surface(SurfaceId(1))
            .and_then(|s| s.role.clone())
            .as_deref(),
        Some("xdg_toplevel")
    );
    assert_eq!(
        tl.clients().next().and_then(|c| c.pid),
        Some(ProcessId(4242))
    );
    assert_eq!(
        tl.process(ProcessId(4242)).map(|p| p.command.as_str()),
        Some("app --flag")
    );
    assert_eq!(tl.outputs().count(), 1);
    let vb = tl.vblanks(OutputId(0));
    assert_eq!(vb.len(), 4);
    assert_eq!(vb[2].lateness(), Nanos(20_000));
    assert_eq!(tl.orphans(), 0);
}

#[test]
fn presented_sequences() {
    let tl = FrameTimeline::build(&steady(5).build());
    let p = tl.presented_frames(SurfaceId(1), None);
    assert_eq!(p.len(), 5);
    assert_eq!(p[0].interval, None);
    assert!(p[1..].iter().all(|f| f.vblanks_missed == Some(0)));
    assert_eq!(tl.presented_on(OutputId(0)).len(), 5);
    assert!(tl.presented_on(OutputId(9)).is_empty());
    let next = tl
        .next_presented(SurfaceId(1), vblank_time(3))
        .map(|f| f.commit);
    assert_eq!(next, Some(CommitId(3)));
    assert_eq!(
        tl.frames_in(SurfaceId(1), vblank_time(2)..vblank_time(4))
            .count(),
        2
    );
}

#[test]
fn stage_records_of_unknown_commits_count_as_orphans() {
    let mut f = steady(2);
    f.ready(vblank_time(5), 1, 99);
    let tl = FrameTimeline::build(&f.build());
    assert_eq!(tl.orphans(), 1);
    assert!(tl.frame(CommitId(99)).is_none());
}
