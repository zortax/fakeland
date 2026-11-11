//! Property tests: random configurations and random client behavior keep the scheduler
//! invariants.

use std::collections::{BTreeMap, BTreeSet};

use fakeland_timing::{
    Action, CallbackPolicy, Perturbation, Scheduler, TimingConfig, TimingMode, Update,
};
use fakeland_types::{CommitId, Hz, Nanos, SurfaceId, Timestamp};
use proptest::prelude::*;

const START: Timestamp = Timestamp(1_000_000_000);

#[derive(Clone, Debug)]
enum Op {
    Submit {
        surface: u64,
        content: bool,
        callback: bool,
        set_barrier: bool,
        wait_barrier: bool,
        pending: bool,
        ready_delay_us: i64,
        target_us: Option<i64>,
    },
    Advance(i64),
    MarkReady(usize),
    Step(u32),
}

fn op() -> impl Strategy<Value = Op> {
    prop_oneof![
        4 => (
            1u64..4,
            prop::bool::weighted(0.7),
            any::<bool>(),
            any::<bool>(),
            any::<bool>(),
            prop::bool::weighted(0.2),
            0i64..20_000,
            prop::option::weighted(0.2, 0i64..120_000),
        )
            .prop_map(
                |(surface, content, callback, set_barrier, wait_barrier, pending, ready, target)| {
                    Op::Submit {
                        surface,
                        content,
                        callback,
                        set_barrier,
                        wait_barrier,
                        pending,
                        ready_delay_us: ready,
                        target_us: target,
                    }
                }
            ),
        3 => (0i64..25_000).prop_map(Op::Advance),
        1 => (0usize..8).prop_map(Op::MarkReady),
        1 => (1u32..4).prop_map(Op::Step),
    ]
}

fn config() -> impl Strategy<Value = TimingConfig> {
    let mode = prop_oneof![
        prop::sample::select(vec![30.0, 59.94, 60.0, 144.0])
            .prop_map(|hz| TimingMode::fixed(Hz(hz))),
        (30.0f64..60.0, 90.0f64..240.0).prop_map(|(a, b)| TimingMode::vrr(Hz(a), Hz(b))),
        Just(TimingMode::Unthrottled),
        Just(TimingMode::manual(Hz(60.0))),
    ];
    let policy = prop_oneof![
        Just(CallbackPolicy::AtVblank),
        Just(CallbackPolicy::OnPresent),
        (0i64..40_000).prop_map(|us| CallbackPolicy::BeforeVblank {
            lead: Nanos::from_micros(us)
        }),
    ];
    let perturbation = prop::option::of(
        (
            0i64..2_000,
            prop::option::of(2u32..6),
            0.0f64..0.3,
            any::<u64>(),
        )
            .prop_map(|(jitter, every, p, seed)| Perturbation {
                latch_jitter: Nanos::from_micros(jitter),
                drop_every: every,
                drop_probability: p,
                seed,
            }),
    );
    (mode, policy, perturbation, 0i64..3_000, any::<bool>()).prop_map(
        |(mode, callbacks, perturbation, offset_us, zero_copy)| TimingConfig {
            mode,
            latch_offset: Nanos::from_micros(offset_us),
            callbacks,
            perturbation,
            zero_copy,
        },
    )
}

struct Run {
    log: Vec<Action>,
    submitted: Vec<Update>,
    on_screen: BTreeMap<SurfaceId, Option<CommitId>>,
}

fn run(cfg: TimingConfig, ops: &[Op]) -> Run {
    let manual = matches!(cfg.mode, TimingMode::Manual { .. });
    let mut s = Scheduler::new(cfg, START).unwrap_or_else(|e| panic!("{e}"));
    let mut now = START;
    let mut log = Vec::new();
    let mut submitted = Vec::new();
    let mut pending: Vec<CommitId> = Vec::new();
    let mut next = 1u64;
    let tick = |s: &mut Scheduler, now: Timestamp, n: u32, log: &mut Vec<Action>| {
        if manual {
            log.extend(s.step(n, now).unwrap_or_default());
        } else {
            log.extend(s.advance(now));
        }
    };
    for op in ops {
        match *op {
            Op::Submit {
                surface,
                content,
                callback,
                set_barrier,
                wait_barrier,
                pending: is_pending,
                ready_delay_us,
                target_us,
            } => {
                let mut u = Update::new(SurfaceId(surface), CommitId(next))
                    .content(content)
                    .callback(callback)
                    .fifo(set_barrier, wait_barrier)
                    .ready_at(now + Nanos::from_micros(ready_delay_us));
                if let Some(t) = target_us {
                    u = u.target(now + Nanos::from_micros(t));
                }
                if is_pending {
                    u = u.pending();
                    pending.push(u.commit);
                }
                next += 1;
                s.submit(now, u);
                submitted.push(u);
            }
            Op::Advance(us) => {
                now += Nanos::from_micros(us);
                log.extend(s.advance(now));
            }
            Op::MarkReady(i) => {
                if !pending.is_empty() {
                    let c = pending.remove(i % pending.len());
                    s.mark_ready(c, now);
                }
            }
            Op::Step(n) => tick(&mut s, now, n, &mut log),
        }
    }
    for c in pending.drain(..) {
        s.mark_ready(c, now);
    }
    for _ in 0..400 {
        now += Nanos::from_millis(25);
        tick(&mut s, now, 1, &mut log);
    }
    let on_screen = submitted
        .iter()
        .map(|u| (u.surface, s.on_screen(u.surface)))
        .collect();
    for u in &submitted {
        assert_eq!(
            s.unfinished(u.surface),
            0,
            "surface {} has unfinished updates",
            u.surface
        );
    }
    Run {
        log,
        submitted,
        on_screen,
    }
}

fn check(run: &Run) -> Result<(), TestCaseError> {
    let mut terminal: BTreeMap<CommitId, usize> = BTreeMap::new();
    let mut latched_or_discarded: BTreeSet<CommitId> = BTreeSet::new();
    let mut called: BTreeSet<CommitId> = BTreeSet::new();
    let mut released: BTreeSet<CommitId> = BTreeSet::new();
    let mut last_seq: BTreeMap<SurfaceId, u64> = BTreeMap::new();
    let mut vblank: Option<(u64, Timestamp)> = None;
    let mut last_time = Timestamp::ZERO;
    let mut unshown: BTreeMap<SurfaceId, CommitId> = BTreeMap::new();
    let by_commit: BTreeMap<CommitId, Update> =
        run.submitted.iter().map(|u| (u.commit, *u)).collect();

    for a in &run.log {
        match a {
            Action::Vblank { seq, ideal, .. } => {
                let expected = vblank.map_or(1, |v| v.0 + 1);
                prop_assert_eq!(*seq, expected);
                prop_assert!(*ideal >= last_time);
                last_time = *ideal;
                vblank = Some((*seq, *ideal));
            }
            Action::Latched { commit, .. } => {
                prop_assert!(!terminal.contains_key(commit));
                latched_or_discarded.insert(*commit);
            }
            Action::Presented {
                surface,
                commit,
                seq,
                time,
                content,
                ..
            } => {
                let expected = if by_commit[commit].has_content {
                    Some(*commit)
                } else {
                    unshown.get(surface).copied()
                };
                unshown.remove(surface);
                prop_assert_eq!(*content, expected);
                *terminal.entry(*commit).or_default() += 1;
                prop_assert!(latched_or_discarded.contains(commit));
                prop_assert_eq!(Some((*seq, *time)), vblank);
                let prev = last_seq.insert(*surface, *seq);
                prop_assert!(
                    prev.is_none_or(|p| p < *seq),
                    "seq {} after {:?}",
                    seq,
                    prev
                );
                let u = by_commit[commit];
                prop_assert!(u.target.is_none_or(|t| t <= *time));
            }
            Action::Discarded { surface, commit } => {
                if by_commit[commit].has_content {
                    unshown.insert(*surface, *commit);
                }
                *terminal.entry(*commit).or_default() += 1;
                latched_or_discarded.insert(*commit);
            }
            Action::Released { commit, .. } => {
                prop_assert!(by_commit[commit].has_content);
                prop_assert!(terminal.contains_key(commit));
                prop_assert!(released.insert(*commit), "released twice");
            }
            Action::FrameCallbacks { time, callbacks } => {
                prop_assert!(*time >= last_time);
                last_time = *time;
                for c in callbacks {
                    prop_assert!(latched_or_discarded.contains(&c.commit));
                    prop_assert!(by_commit[&c.commit].wants_callback);
                    prop_assert!(called.insert(c.commit), "callback twice");
                }
            }
            Action::BarrierCleared { .. } => {}
        }
    }
    for u in &run.submitted {
        prop_assert_eq!(
            terminal.get(&u.commit).copied(),
            Some(1),
            "commit {}",
            u.commit
        );
        prop_assert_eq!(called.contains(&u.commit), u.wants_callback);
        let held = run.on_screen[&u.surface] == Some(u.commit);
        prop_assert_eq!(released.contains(&u.commit), u.has_content && !held);
    }
    Ok(())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn scheduler_invariants(cfg in config(), ops in prop::collection::vec(op(), 0..60)) {
        prop_assume!(cfg.validate().is_ok());
        let run = run(cfg, &ops);
        check(&run)?;
    }
}
