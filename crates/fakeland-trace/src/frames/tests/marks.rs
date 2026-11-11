//! App marks: frame ids, spans, named points and exact input attribution.

use fakeland_types::{CommitId, InputEventId, Nanos, Timestamp};

use crate::event::{AppMarkKind, InputKind, SyncKind};
use crate::frames::{Attribution, FrameTimeline};
use crate::testing::{Fixture, vblank_time};

fn fixture() -> Fixture {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    let t0 = vblank_time(0);
    f.input(t0 + Nanos::from_millis(1), 1, 1, InputKind::Key);
    f.input(t0 + Nanos::from_millis(2), 2, 1, InputKind::Key);
    // Marks with a commit id come before their commit record.
    f.mark(
        t0 + Nanos::from_millis(3),
        Some(1),
        Some(1),
        AppMarkKind::Frame { frame_id: 77 },
    );
    f.mark(
        t0 + Nanos::from_millis(3),
        Some(1),
        Some(1),
        AppMarkKind::Span {
            name: "render".into(),
            begin: t0 + Nanos::from_millis(1),
            end: t0 + Nanos::from_millis(3),
        },
    );
    f.commit(t0 + Nanos::from_millis(3), 1, 1, SyncKind::None);
    f.present(1, 1, 1);
    // The app reports that commit 2 handles input 1, and commit 3 handles input 2.
    let t1 = vblank_time(1);
    f.mark(
        t1,
        Some(1),
        None,
        AppMarkKind::InputHandled {
            input: InputEventId(1),
        },
    );
    f.mark(
        t1,
        Some(1),
        None,
        AppMarkKind::Mark {
            name: "scrolled".into(),
        },
    );
    f.commit(t1 + Nanos::from_millis(1), 1, 2, SyncKind::None);
    f.discard(t1 + Nanos::from_millis(2), 1, 2);
    f.commit(t1 + Nanos::from_millis(2), 1, 3, SyncKind::None);
    f.mark(
        t1 + Nanos::from_millis(2),
        Some(1),
        Some(3),
        AppMarkKind::InputHandled {
            input: InputEventId(2),
        },
    );
    f.present(1, 3, 2);
    f.mark(
        Timestamp(5),
        None,
        None,
        AppMarkKind::Mark {
            name: "global".into(),
        },
    );
    f
}

#[test]
fn marks_attach_to_their_commits() {
    let tl = FrameTimeline::build(&fixture().build());
    let f1 = tl.frame(CommitId(1)).unwrap();
    assert_eq!(f1.marks.frame_id, Some(77));
    assert_eq!(f1.span("render"), Some(Nanos::from_millis(2)));
    assert_eq!(f1.inputs, vec![InputEventId(1), InputEventId(2)]);
    let f2 = tl.frame(CommitId(2)).unwrap();
    assert_eq!(f2.marks.inputs_handled, vec![InputEventId(1)]);
    assert_eq!(f2.marks.names, vec!["scrolled".to_string()]);
    let f3 = tl.frame(CommitId(3)).unwrap();
    assert_eq!(f3.marks.inputs_handled, vec![InputEventId(2)]);
    assert_eq!(tl.global_marks(), 1);
}

#[test]
fn marks_beat_the_next_commit_heuristic() {
    let tl = FrameTimeline::build(&fixture().build());
    let i1 = tl.input(InputEventId(1)).unwrap();
    assert_eq!(i1.handled_by, Some(CommitId(2)));
    assert_eq!(i1.attribution, Attribution::Mark);
    // Commit 2 drops, so commit 3 first shows the effect.
    assert_eq!(i1.presented_commit, Some(CommitId(3)));
    assert_eq!(i1.presented, Some(vblank_time(2)));
    let i2 = tl.input(InputEventId(2)).unwrap();
    assert_eq!(i2.handled_by, Some(CommitId(3)));
    assert_eq!(i2.attribution, Attribution::Mark);
}

#[test]
fn without_marks_the_next_commit_handles_the_input() {
    let trace = fixture().build();
    let unmarked = trace.filter(|r| r.event.name() != "app_mark");
    let tl = FrameTimeline::build(&unmarked);
    let i1 = tl.input(InputEventId(1)).unwrap();
    assert_eq!(i1.handled_by, Some(CommitId(1)));
    assert_eq!(i1.attribution, Attribution::NextCommit);
    assert_eq!(i1.presented, Some(vblank_time(1)));
}
