//! Fixed-grid scheduling: latch, discard, fifo, commit timing, callbacks, drops and jitter.

mod harness;

use fakeland_timing::{
    Action, CallbackPolicy, FrameCallback, Perturbation, PresentFlags, Scheduler, TimingConfig,
    Update,
};
use fakeland_types::{CommitId, Hz, Nanos, SurfaceId};
use harness::{FifoClient, Mailbox, P60, PacedClient, Renderer, START, Sim, ms, t60, us};

const S: SurfaceId = SurfaceId(1);

fn sched(cfg: TimingConfig) -> Scheduler {
    Scheduler::new(cfg, START).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn first_vblank_is_one_period_after_start() {
    let s = sched(TimingConfig::default());
    assert_eq!(s.refresh(), Some(P60));
    assert_eq!(s.latch_deadline(1), Some(t60(1) - ms(1)));
    assert_eq!(s.next_wakeup(), Some(t60(1) - ms(1)));
    assert_eq!(
        s.next_vblank_after(START).map(|p| (p.seq, p.time)),
        Some((1, t60(1)))
    );
    assert_eq!(s.seq_at(t60(3) + ms(2)), Some(3));
    assert_eq!(s.phase_of(t60(3) + ms(2)), Some(ms(2)));
    assert_eq!(s.vblank_time(7), Some(t60(7)));
}

#[test]
fn idle_output_still_ticks() {
    let mut sim = Sim::new(TimingConfig::default());
    sim.run_until(t60(3));
    assert_eq!(
        sim.vblanks(),
        vec![(1, t60(1), false), (2, t60(2), false), (3, t60(3), false)]
    );
}

#[test]
fn exact_sequence_of_one_frame() {
    let mut s = sched(TimingConfig::default().with_zero_copy(true));
    s.submit(START, Update::new(S, CommitId(1)));
    let out = s.advance(t60(1));
    let flags = PresentFlags::VSYNC
        | PresentFlags::HW_CLOCK
        | PresentFlags::HW_COMPLETION
        | PresentFlags::ZERO_COPY;
    assert_eq!(
        out,
        vec![
            Action::Latched {
                surface: S,
                commit: CommitId(1),
                seq: 1
            },
            Action::Vblank {
                seq: 1,
                ideal: t60(1),
                woke: t60(1),
                dropped: false
            },
            Action::Presented {
                surface: S,
                commit: CommitId(1),
                seq: 1,
                time: t60(1),
                refresh: Some(P60),
                flags,
                content: Some(CommitId(1))
            },
            Action::FrameCallbacks {
                time: t60(1),
                callbacks: vec![FrameCallback {
                    surface: S,
                    commit: CommitId(1)
                }]
            },
        ]
    );
    assert_eq!(s.on_screen(S), Some(CommitId(1)));
    assert_eq!(s.seq(), 1);
}

#[test]
fn late_wake_keeps_ideal_times() {
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)));
    let out = s.advance(t60(1) + us(300));
    assert!(out.contains(&Action::Vblank {
        seq: 1,
        ideal: t60(1),
        woke: t60(1) + us(300),
        dropped: false
    }));
}

#[test]
fn renderer_that_fits_presents_every_vblank() {
    let mut sim = Sim::new(TimingConfig::default());
    sim.add(Renderer::new(S, ms(5), START));
    sim.run_until(t60(10));
    let presented = sim.presented(S);
    assert_eq!(
        presented,
        (1..=10).map(|n| (n, t60(n as i64))).collect::<Vec<_>>()
    );
    assert_eq!(sim.callback_times(S), (1..=10).map(t60).collect::<Vec<_>>());
    assert!(sim.discarded(S).is_empty());
}

#[test]
fn renderer_that_misses_the_latch_halves_the_rate() {
    let mut sim = Sim::new(TimingConfig::default());
    sim.add(Renderer::new(S, ms(16), START));
    sim.run_until(t60(10));
    assert_eq!(sim.presented_seqs(S), vec![1, 3, 5, 7, 9]);
}

#[test]
fn gpu_time_counts_against_the_latch() {
    let mut sim = Sim::new(TimingConfig::default());
    let mut r = Renderer::new(S, ms(10), START);
    r.gpu = ms(6);
    sim.add(r);
    sim.run_until(t60(9));
    assert_eq!(sim.presented_seqs(S), vec![1, 3, 5, 7, 9]);
}

#[test]
fn before_vblank_dispatch_gives_a_render_window() {
    let lead = ms(8);
    let cfg = TimingConfig::default().with_callbacks(CallbackPolicy::BeforeVblank { lead });
    let mut sim = Sim::new(cfg);
    sim.add(Renderer::new(S, ms(5), START));
    sim.run_until(t60(9));
    assert_eq!(sim.presented_seqs(S), (1..=9).collect::<Vec<_>>());
    assert_eq!(
        sim.callback_times(S),
        (2..=9).map(|n| t60(n) - lead).collect::<Vec<_>>()
    );

    let mut sim = Sim::new(cfg);
    sim.add(Renderer::new(S, us(7_500), START));
    sim.run_until(t60(9));
    assert_eq!(sim.presented_seqs(S), vec![1, 3, 5, 7, 9]);
}

#[test]
fn mailbox_client_discards_all_but_the_newest() {
    let mut sim = Sim::new(TimingConfig::default());
    sim.add(Mailbox::new(S, ms(4), START, t60(2)));
    sim.run_until(t60(3));
    // Commits at 0, 4, 8 and 12 ms make the latch at 15.67 ms; 16 to 32 ms make the next one.
    assert_eq!(sim.presented_commits(S), vec![CommitId(4), CommitId(9)]);
    assert_eq!(
        sim.discarded(S),
        [1, 2, 3, 5, 6, 7, 8].map(CommitId).to_vec()
    );
    let mut released = sim.released(S);
    released.sort();
    assert_eq!(released, [1, 2, 3, 4, 5, 6, 7, 8].map(CommitId).to_vec());
    assert_eq!(sim.sched.on_screen(S), Some(CommitId(9)));
}

#[test]
fn fifo_client_presents_one_update_per_refresh() {
    let mut sim = Sim::new(TimingConfig::default());
    sim.add(FifoClient::new(S, 3, 6, START));
    sim.run_until(t60(8));
    assert_eq!(
        sim.presented(S),
        (1..=6).map(|n| (n, t60(n as i64))).collect::<Vec<_>>()
    );
    assert_eq!(
        sim.presented_commits(S),
        (1..=6).map(CommitId).collect::<Vec<_>>()
    );
    assert!(sim.discarded(S).is_empty());
    let cleared = sim
        .log
        .iter()
        .filter(|(_, a)| matches!(a, Action::BarrierCleared { .. }))
        .count();
    assert_eq!(cleared, 6);
}

#[test]
fn update_without_wait_flag_skips_the_barrier() {
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)).fifo(true, true));
    s.submit(START, Update::new(S, CommitId(2)));
    let out = s.advance(t60(1));
    assert!(out.contains(&Action::Discarded {
        surface: S,
        commit: CommitId(1)
    }));
    assert!(out.contains(&Action::BarrierCleared { surface: S }));
    assert_eq!(s.on_screen(S), Some(CommitId(2)));
}

#[test]
fn commit_timing_client_presents_at_its_targets() {
    let mut sim = Sim::new(TimingConfig::default());
    sim.add(PacedClient::new(S, P60, 2, START));
    sim.run_until(t60(8));
    assert_eq!(sim.presented_seqs(S), vec![2, 4, 6, 8]);
}

#[test]
fn target_between_vblanks_presents_at_the_next_one() {
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)).target(t60(3) + Nanos(1)));
    s.submit(START, Update::new(S, CommitId(2)));
    let out = s.advance(t60(5));
    let presented: Vec<_> = out
        .iter()
        .filter_map(|a| match a {
            Action::Presented { commit, seq, .. } => Some((*commit, *seq)),
            _ => None,
        })
        .collect();
    // The second update waits behind the first, then both latch together at vblank 4.
    assert_eq!(presented, vec![(CommitId(2), 4)]);
    assert!(out.contains(&Action::Discarded {
        surface: S,
        commit: CommitId(1)
    }));
}

#[test]
fn pending_fence_decides_the_vblank() {
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)).pending());
    s.advance(t60(1) - ms(1) - Nanos(1));
    assert!(s.mark_ready(CommitId(1), t60(1) - ms(1)));
    let out = s.advance(t60(1));
    assert!(
        out.iter()
            .any(|a| matches!(a, Action::Presented { seq: 1, .. }))
    );

    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)).pending());
    s.advance(t60(1) - us(500));
    assert!(s.mark_ready(CommitId(1), t60(1) - us(500)));
    let out = s.advance(t60(2));
    let seqs: Vec<u64> = out
        .iter()
        .filter_map(|a| match a {
            Action::Presented { seq, .. } => Some(*seq),
            _ => None,
        })
        .collect();
    assert_eq!(seqs, vec![2]);
    assert!(!s.mark_ready(CommitId(99), t60(2)));
}

#[test]
fn pending_update_blocks_the_updates_behind_it() {
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)).pending());
    s.submit(START, Update::new(S, CommitId(2)));
    let out = s.advance(t60(2));
    assert!(!out.iter().any(|a| matches!(a, Action::Presented { .. })));
    s.mark_ready(CommitId(1), t60(2));
    let out = s.advance(t60(3));
    assert!(out.contains(&Action::Discarded {
        surface: S,
        commit: CommitId(1)
    }));
    assert!(out.iter().any(|a| matches!(
        a,
        Action::Presented {
            commit: CommitId(2),
            seq: 3,
            ..
        }
    )));
}

#[test]
fn replaced_buffer_is_released_at_the_vblank_that_replaces_it() {
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)));
    s.advance(t60(1));
    s.submit(t60(1) + ms(2), Update::new(S, CommitId(2)));
    let out = s.advance(t60(2) - Nanos(1));
    assert!(!out.iter().any(|a| matches!(a, Action::Released { .. })));
    let out = s.advance(t60(2));
    assert!(out.contains(&Action::Released {
        surface: S,
        commit: CommitId(1)
    }));
}

#[test]
fn update_without_content_keeps_the_buffer() {
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)));
    s.submit(START + ms(1), Update::callback_only(S, CommitId(2)));
    let out = s.advance(t60(1));
    assert!(out.contains(&Action::Discarded {
        surface: S,
        commit: CommitId(1)
    }));
    assert!(out.iter().any(|a| matches!(
        a,
        Action::Presented {
            commit: CommitId(2),
            content: Some(CommitId(1)),
            ..
        }
    )));
    assert!(out.contains(&Action::FrameCallbacks {
        time: t60(1),
        callbacks: vec![
            FrameCallback {
                surface: S,
                commit: CommitId(1)
            },
            FrameCallback {
                surface: S,
                commit: CommitId(2)
            }
        ]
    }));
    assert_eq!(s.on_screen(S), Some(CommitId(1)));
    s.submit(t60(1) + ms(1), Update::new(S, CommitId(3)));
    let out = s.advance(t60(2));
    assert!(out.contains(&Action::Released {
        surface: S,
        commit: CommitId(1)
    }));
}

#[test]
fn callback_only_after_the_latch_fires_at_the_coming_vblank() {
    let submit = t60(1) - us(500);
    let mut s = sched(TimingConfig::default());
    s.advance(submit);
    s.submit(submit, Update::callback_only(S, CommitId(1)));
    let out = s.advance(t60(1));
    assert_eq!(
        out[0],
        Action::Latched {
            surface: S,
            commit: CommitId(1),
            seq: 2
        }
    );
    assert!(out.contains(&Action::FrameCallbacks {
        time: t60(1),
        callbacks: vec![FrameCallback {
            surface: S,
            commit: CommitId(1)
        }]
    }));
    let out = s.advance(t60(2));
    assert!(out.iter().any(|a| matches!(
        a,
        Action::Presented {
            commit: CommitId(1),
            seq: 2,
            content: None,
            ..
        }
    )));
    assert!(
        !out.iter()
            .any(|a| matches!(a, Action::FrameCallbacks { .. }))
    );
}

#[test]
fn presented_content_names_the_new_buffer_of_the_refresh() {
    let content = |out: &[Action]| {
        out.iter().find_map(|a| match a {
            Action::Presented {
                commit, content, ..
            } => Some((*commit, *content)),
            _ => None,
        })
    };
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::callback_only(S, CommitId(1)));
    let out = s.advance(t60(1));
    assert_eq!(content(&out), Some((CommitId(1), None)));

    s.submit(t60(1) + ms(1), Update::new(S, CommitId(2)));
    s.submit(t60(1) + ms(2), Update::callback_only(S, CommitId(3)));
    let out = s.advance(t60(2));
    assert_eq!(content(&out), Some((CommitId(3), Some(CommitId(2)))));
    assert_eq!(s.on_screen(S), Some(CommitId(2)));

    s.submit(t60(2) + ms(1), Update::new(S, CommitId(4)));
    s.submit(t60(2) + ms(2), Update::new(S, CommitId(5)));
    let out = s.advance(t60(3));
    assert_eq!(content(&out), Some((CommitId(5), Some(CommitId(5)))));
}

#[test]
fn on_present_policy_fires_callback_only_requests_at_once() {
    let submit = START + ms(3);
    let mut s = sched(TimingConfig::default().with_callbacks(CallbackPolicy::OnPresent));
    s.advance(submit);
    s.submit(submit, Update::callback_only(S, CommitId(1)));
    let out = s.advance(submit);
    assert_eq!(
        out,
        vec![
            Action::Latched {
                surface: S,
                commit: CommitId(1),
                seq: 1
            },
            Action::FrameCallbacks {
                time: submit,
                callbacks: vec![FrameCallback {
                    surface: S,
                    commit: CommitId(1)
                }]
            }
        ]
    );
}

#[test]
fn before_vblank_policy_delays_callback_only_requests_to_the_dispatch() {
    let lead = ms(4);
    let submit = t60(1) - us(500);
    let mut s =
        sched(TimingConfig::default().with_callbacks(CallbackPolicy::BeforeVblank { lead }));
    s.advance(submit);
    s.submit(submit, Update::callback_only(S, CommitId(1)));
    assert_eq!(s.next_wakeup(), Some(submit));
    s.advance(submit);
    assert_eq!(s.next_wakeup(), Some(t60(1)));
    s.advance(t60(1));
    assert_eq!(s.next_wakeup(), Some(t60(2) - lead));
    let out = s.advance(t60(2) - lead);
    assert_eq!(
        out,
        vec![Action::FrameCallbacks {
            time: t60(2) - lead,
            callbacks: vec![FrameCallback {
                surface: S,
                commit: CommitId(1)
            }]
        }]
    );
}

#[test]
fn dropped_vblanks_latch_nothing_and_keep_the_grid() {
    let cfg = TimingConfig::default().with_perturbation(Perturbation {
        drop_every: Some(3),
        ..Default::default()
    });
    let mut sim = Sim::new(cfg);
    sim.add(Renderer::new(S, ms(5), START));
    sim.run_until(t60(9));
    let dropped: Vec<u64> = sim
        .vblanks()
        .into_iter()
        .filter(|v| v.2)
        .map(|v| v.0)
        .collect();
    assert_eq!(dropped, vec![3, 6, 9]);
    assert_eq!(sim.presented_seqs(S), vec![1, 2, 4, 5, 7, 8]);
    assert!(sim.presented(S).iter().all(|(n, t)| *t == t60(*n as i64)));
    assert_eq!(sim.callback_times(S), [1, 2, 4, 5, 7, 8].map(t60).to_vec());
}

#[test]
fn jitter_moves_the_latch_deadline() {
    let cfg = TimingConfig::default().with_perturbation(Perturbation {
        latch_jitter: us(500),
        seed: 42,
        ..Default::default()
    });
    let probe = sched(cfg);
    let mut distinct = std::collections::BTreeSet::new();
    for n in 1..40u64 {
        let latch = probe.latch_deadline(n).unwrap_or_default();
        let nominal = t60(n as i64) - ms(1);
        assert!(latch >= nominal - us(500) && latch <= nominal + us(500));
        distinct.insert(latch - nominal);
    }
    assert!(distinct.len() > 30);
    for n in [1u64, 2, 5, 11] {
        let latch = probe.latch_deadline(n).unwrap_or_default();
        for (ready, expected) in [(latch, n), (latch + Nanos(1), n + 1)] {
            let mut s = sched(cfg);
            s.advance(t60(n as i64 - 1));
            s.submit(
                t60(n as i64 - 1),
                Update::new(S, CommitId(1)).ready_at(ready),
            );
            let out = s.advance(t60(n as i64 + 1));
            let seq = out.iter().find_map(|a| match a {
                Action::Presented { seq, .. } => Some(*seq),
                _ => None,
            });
            assert_eq!(seq, Some(expected), "vblank {n}, ready {ready}");
        }
    }
}

#[test]
fn fractional_rates_stay_exact_over_long_runs() {
    let mut s = sched(TimingConfig::fixed(Hz(59.94)));
    let period = Hz(59.94).period();
    let out = s.advance(START + Nanos::from_secs(10));
    let last = out.iter().rev().find_map(|a| match a {
        Action::Vblank { seq, ideal, .. } => Some((*seq, *ideal)),
        _ => None,
    });
    let (seq, ideal) = last.unwrap_or_default();
    assert_eq!(seq, 599);
    assert_eq!(ideal, START + Nanos(period.as_nanos() * 599));
}

#[test]
fn two_surfaces_latch_independently() {
    let a = SurfaceId(1);
    let b = SurfaceId(2);
    let mut sim = Sim::new(TimingConfig::default());
    sim.add(Renderer::new(a, ms(4), START));
    sim.add(Renderer::new(b, ms(20), START));
    sim.run_until(t60(6));
    assert_eq!(sim.presented_seqs(a), vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(sim.presented_seqs(b), vec![1, 3, 5]);
}
