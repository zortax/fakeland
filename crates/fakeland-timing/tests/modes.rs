//! VRR, unthrottled and manual scheduling.

mod harness;

use fakeland_timing::{
    Action, CallbackPolicy, FrameCallback, Perturbation, PresentFlags, ScheduleError, Scheduler,
    TimingConfig, Update,
};
use fakeland_types::{CommitId, Hz, Nanos, SurfaceId, Timestamp};
use harness::{FifoClient, P60, Renderer, START, Sim, ms};

const S: SurfaceId = SurfaceId(1);
const MIN_P: Nanos = Nanos(6_944_444);
const MAX_P: Nanos = Nanos(20_833_333);

fn sched(cfg: TimingConfig) -> Scheduler {
    Scheduler::new(cfg, START).unwrap_or_else(|e| panic!("{e}"))
}

fn vrr() -> TimingConfig {
    TimingConfig::vrr(Hz(48.0), Hz(144.0))
}

fn presents(out: &[Action]) -> Vec<(CommitId, u64, Timestamp)> {
    out.iter()
        .filter_map(|a| match a {
            Action::Presented {
                commit, seq, time, ..
            } => Some((*commit, *seq, *time)),
            _ => None,
        })
        .collect()
}

#[test]
fn vrr_presents_after_the_shortest_period() {
    let mut s = sched(vrr());
    s.submit(START + ms(3), Update::new(S, CommitId(1)));
    assert_eq!(s.next_wakeup(), Some(START + MIN_P));
    let out = s.advance(START + MIN_P);
    assert_eq!(
        out,
        vec![
            Action::Vblank {
                seq: 1,
                ideal: START + MIN_P,
                woke: START + MIN_P,
                dropped: false
            },
            Action::Latched {
                surface: S,
                commit: CommitId(1),
                seq: 1
            },
            Action::Presented {
                surface: S,
                commit: CommitId(1),
                seq: 1,
                time: START + MIN_P,
                refresh: Some(MIN_P),
                flags: PresentFlags::VSYNC | PresentFlags::HW_CLOCK | PresentFlags::HW_COMPLETION,
                content: Some(CommitId(1))
            },
            Action::FrameCallbacks {
                time: START + MIN_P,
                callbacks: vec![FrameCallback {
                    surface: S,
                    commit: CommitId(1)
                }]
            }
        ]
    );
}

#[test]
fn vrr_presents_a_late_update_at_once() {
    let mut s = sched(vrr());
    let at = START + ms(10);
    s.submit(START, Update::new(S, CommitId(1)).ready_at(at));
    let out = s.advance(at);
    assert_eq!(presents(&out), vec![(CommitId(1), 1, at)]);
}

#[test]
fn vrr_repeats_the_frame_after_the_longest_period() {
    let mut s = sched(vrr());
    let out = s.advance(START + MAX_P * 2);
    let vblanks: Vec<_> = out
        .iter()
        .filter_map(|a| match a {
            Action::Vblank { seq, ideal, .. } => Some((*seq, *ideal)),
            _ => None,
        })
        .collect();
    assert_eq!(vblanks, vec![(1, START + MAX_P), (2, START + MAX_P * 2)]);
    assert!(presents(&out).is_empty());

    s.submit(
        START + MAX_P * 2 + ms(1),
        Update::callback_only(S, CommitId(1)),
    );
    let out = s.advance(START + MAX_P * 3);
    assert!(out.contains(&Action::FrameCallbacks {
        time: START + MAX_P * 3,
        callbacks: vec![FrameCallback {
            surface: S,
            commit: CommitId(1)
        }]
    }));
    assert_eq!(presents(&out), vec![(CommitId(1), 3, START + MAX_P * 3)]);
}

#[test]
fn vrr_follows_the_client_rate_inside_the_range() {
    let mut sim = Sim::new(vrr());
    sim.add(Renderer::new(S, ms(10), START));
    sim.run_until(START + ms(100));
    let times: Vec<Timestamp> = sim.presented(S).into_iter().map(|p| p.1).collect();
    assert_eq!(times[0], START + MIN_P);
    for w in times.windows(2) {
        assert_eq!(w[1] - w[0], ms(10));
    }

    let mut sim = Sim::new(vrr());
    sim.add(Renderer::new(S, ms(2), START));
    sim.run_until(START + ms(100));
    let times: Vec<Timestamp> = sim.presented(S).into_iter().map(|p| p.1).collect();
    for w in times.windows(2) {
        assert_eq!(w[1] - w[0], MIN_P);
    }
}

#[test]
fn vrr_slow_client_gets_repeated_frames() {
    let mut sim = Sim::new(vrr());
    sim.add(Renderer::new(S, ms(30), START));
    sim.run_until(START + ms(80));
    let vblanks = sim.vblanks();
    let presented: Vec<u64> = sim.presented_seqs(S);
    assert_eq!(vblanks[0].1, START + MIN_P);
    assert_eq!(vblanks[1].1, START + MIN_P + MAX_P);
    assert_eq!(presented[0], 1);
    // The second commit at 36.9 ms comes after the repeat at 27.8 ms and before the next repeat.
    assert_eq!(presented[1], 3);
    assert_eq!(sim.presented(S)[1].1, START + MIN_P + ms(30));
}

#[test]
fn vrr_drop_repeats_the_frame() {
    let cfg = vrr().with_perturbation(Perturbation {
        drop_every: Some(2),
        ..Default::default()
    });
    let mut s = sched(cfg);
    s.submit(START, Update::new(S, CommitId(1)));
    s.advance(START + MIN_P);
    s.submit(START + MIN_P + ms(1), Update::new(S, CommitId(2)));
    let out = s.advance(START + MIN_P * 3);
    assert!(out.contains(&Action::Vblank {
        seq: 2,
        ideal: START + MIN_P * 2,
        woke: START + MIN_P * 3,
        dropped: true
    }));
    assert_eq!(presents(&out), vec![(CommitId(2), 3, START + MIN_P * 3)]);
}

#[test]
fn vrr_fifo_presents_one_update_per_frame() {
    let mut sim = Sim::new(vrr());
    sim.add(FifoClient::new(S, 3, 5, START));
    sim.run_until(START + ms(100));
    let times: Vec<Timestamp> = sim.presented(S).into_iter().map(|p| p.1).collect();
    assert_eq!(
        times,
        (1..=5).map(|k| START + MIN_P * k).collect::<Vec<_>>()
    );
}

#[test]
fn unthrottled_presents_at_readiness() {
    let mut s = sched(TimingConfig::unthrottled().with_zero_copy(true));
    assert_eq!(s.next_wakeup(), None);
    assert_eq!(s.grid(), None);
    let ready = START + ms(2);
    s.submit(START, Update::new(S, CommitId(1)).ready_at(ready));
    assert_eq!(s.next_wakeup(), Some(ready));
    let out = s.advance(ready);
    assert_eq!(
        out,
        vec![
            Action::Vblank {
                seq: 1,
                ideal: ready,
                woke: ready,
                dropped: false
            },
            Action::Latched {
                surface: S,
                commit: CommitId(1),
                seq: 1
            },
            Action::Presented {
                surface: S,
                commit: CommitId(1),
                seq: 1,
                time: ready,
                refresh: None,
                flags: PresentFlags::HW_CLOCK
                    | PresentFlags::HW_COMPLETION
                    | PresentFlags::ZERO_COPY,
                content: Some(CommitId(1))
            },
            Action::FrameCallbacks {
                time: ready,
                callbacks: vec![FrameCallback {
                    surface: S,
                    commit: CommitId(1)
                }]
            }
        ]
    );
}

#[test]
fn unthrottled_client_runs_at_its_render_rate() {
    let mut sim = Sim::new(TimingConfig::unthrottled());
    sim.add(Renderer::new(S, ms(3), START));
    sim.run_until(START + ms(30));
    let times: Vec<Timestamp> = sim.presented(S).into_iter().map(|p| p.1).collect();
    assert_eq!(
        times,
        (0..=10).map(|k| START + ms(3 * k)).collect::<Vec<_>>()
    );
}

#[test]
fn unthrottled_respects_commit_timing_targets() {
    let mut s = sched(TimingConfig::unthrottled());
    let target = START + ms(7);
    s.submit(START, Update::new(S, CommitId(1)).target(target));
    let out = s.advance(START + ms(20));
    assert_eq!(presents(&out), vec![(CommitId(1), 1, target)]);
}

#[test]
fn manual_mode_presents_only_on_step() {
    let mut s = sched(TimingConfig::manual(Hz(60.0)));
    s.submit(START + ms(1), Update::new(S, CommitId(1)));
    assert!(s.advance(START + Nanos::from_secs(1)).is_empty());
    let at = START + Nanos::from_secs(1) + ms(2);
    let out = s.step(1, at).unwrap_or_default();
    assert_eq!(
        out,
        vec![
            Action::Vblank {
                seq: 1,
                ideal: at,
                woke: at,
                dropped: false
            },
            Action::Latched {
                surface: S,
                commit: CommitId(1),
                seq: 1
            },
            Action::Presented {
                surface: S,
                commit: CommitId(1),
                seq: 1,
                time: at,
                refresh: Some(P60),
                flags: PresentFlags::VSYNC | PresentFlags::HW_CLOCK | PresentFlags::HW_COMPLETION,
                content: Some(CommitId(1))
            },
            Action::FrameCallbacks {
                time: at,
                callbacks: vec![FrameCallback {
                    surface: S,
                    commit: CommitId(1)
                }]
            }
        ]
    );
    assert_eq!(s.next_vblank_after(at).map(|p| p.time), Some(at + P60));
}

#[test]
fn manual_steps_drain_a_fifo_queue_one_per_step() {
    let mut s = sched(TimingConfig::manual(Hz(60.0)));
    for c in 1..=3 {
        s.submit(START, Update::new(S, CommitId(c)).fifo(true, true));
    }
    let out = s.step(3, START + ms(5)).unwrap_or_default();
    let p = presents(&out);
    assert_eq!(
        p,
        (1..=3)
            .map(|c| (CommitId(c), c, START + ms(5)))
            .collect::<Vec<_>>()
    );
}

#[test]
fn manual_on_present_callbacks_need_no_step() {
    let cfg = TimingConfig::manual(Hz(60.0)).with_callbacks(CallbackPolicy::OnPresent);
    let mut s = sched(cfg);
    s.submit(START + ms(1), Update::callback_only(S, CommitId(1)));
    let out = s.advance(START + ms(1));
    assert!(
        out.iter()
            .any(|a| matches!(a, Action::FrameCallbacks { .. }))
    );
}

#[test]
fn step_needs_manual_mode() {
    let mut s = sched(TimingConfig::default());
    assert_eq!(s.step(1, START), Err(ScheduleError::NotManual));
    let mut s = sched(TimingConfig::manual(Hz(60.0)));
    s.set_enabled(false, START);
    assert_eq!(s.step(1, START), Err(ScheduleError::Disabled));
}
