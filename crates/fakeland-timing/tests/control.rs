//! Mode switches, output power and surface removal.

mod harness;

use fakeland_timing::{Action, CallbackPolicy, ConfigError, Scheduler, TimingConfig, Update};
use fakeland_types::{CommitId, Hz, Nanos, SurfaceId, Timestamp};
use harness::{P60, Renderer, START, Sim, ms, t60};

const S: SurfaceId = SurfaceId(1);

fn sched(cfg: TimingConfig) -> Scheduler {
    Scheduler::new(cfg, START).unwrap_or_else(|e| panic!("{e}"))
}

fn vblank_times(out: &[Action]) -> Vec<Timestamp> {
    out.iter()
        .filter_map(|a| match a {
            Action::Vblank { ideal, .. } => Some(*ideal),
            _ => None,
        })
        .collect()
}

#[test]
fn rate_switch_takes_effect_at_the_next_vblank() {
    let mut s = sched(TimingConfig::default());
    let p144 = Hz(144.0).period();
    s.advance(START + ms(5));
    s.reconfigure(TimingConfig::fixed(Hz(144.0)), START + ms(5))
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.period(), Some(P60));
    assert!(s.pending_config().is_some());
    let out = s.advance(t60(1) + p144 * 3);
    assert_eq!(
        vblank_times(&out),
        vec![t60(1), t60(1) + p144, t60(1) + p144 * 2, t60(1) + p144 * 3]
    );
    assert_eq!(s.period(), Some(p144));
    assert!(s.pending_config().is_none());
    assert_eq!(s.seq(), 4);
}

#[test]
fn rate_switch_keeps_a_renderer_running() {
    let mut sim = Sim::new(TimingConfig::default());
    sim.add(Renderer::new(S, ms(2), START));
    sim.run_until(t60(3) + ms(4));
    sim.sched
        .reconfigure(TimingConfig::fixed(Hz(120.0)), sim.now())
        .unwrap_or_else(|e| panic!("{e}"));
    sim.run_until(t60(4) + Hz(120.0).period() * 4);
    let seqs = sim.presented_seqs(S);
    assert_eq!(seqs, (1..=8).collect::<Vec<_>>());
    let times: Vec<Timestamp> = sim.presented(S).into_iter().map(|p| p.1).collect();
    assert_eq!(times[4] - times[3], Hz(120.0).period());
}

#[test]
fn switch_to_vrr_happens_at_the_next_vblank() {
    let mut s = sched(TimingConfig::default());
    s.reconfigure(TimingConfig::vrr(Hz(48.0), Hz(144.0)), START + ms(1))
        .unwrap_or_else(|e| panic!("{e}"));
    let out = s.advance(t60(1) + Hz(48.0).period());
    assert_eq!(vblank_times(&out), vec![t60(1), t60(1) + Hz(48.0).period()]);
}

#[test]
fn switch_from_manual_happens_at_once() {
    let mut s = sched(TimingConfig::manual(Hz(60.0)));
    let at = START + ms(100);
    s.reconfigure(TimingConfig::default(), at)
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.next_vblank_after(at).map(|p| p.time), Some(at + P60));
    let out = s.advance(at + P60);
    assert_eq!(vblank_times(&out), vec![at + P60]);
}

#[test]
fn invalid_config_is_rejected() {
    let mut s = sched(TimingConfig::default());
    let bad = TimingConfig::fixed(Hz(0.0));
    assert_eq!(
        s.reconfigure(bad, START),
        Err(ConfigError::InvalidRate(0.0))
    );
    assert!(Scheduler::new(bad, START).is_err());
}

#[test]
fn disabled_output_holds_updates_and_callbacks() {
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)));
    s.advance(START + ms(5));
    s.set_enabled(false, START + ms(5));
    assert_eq!(s.next_wakeup(), None);
    assert_eq!(s.grid(), None);
    assert!(s.advance(START + ms(500)).is_empty());
    let on = START + ms(500);
    s.set_enabled(true, on);
    let out = s.advance(on + P60);
    assert!(out.iter().any(|a| matches!(
        a,
        Action::Presented {
            commit: CommitId(1),
            seq: 1,
            ..
        }
    )));
    assert_eq!(vblank_times(&out), vec![on + P60]);
}

#[test]
fn disable_after_the_latch_presents_after_power_on() {
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)));
    s.advance(t60(1) - us_(100));
    s.set_enabled(false, t60(1) - us_(100));
    s.submit(t60(1), Update::new(S, CommitId(2)));
    let on = START + ms(200);
    s.set_enabled(true, on);
    let out = s.advance(on + P60);
    assert!(out.contains(&Action::Discarded {
        surface: S,
        commit: CommitId(1)
    }));
    assert!(out.iter().any(|a| matches!(
        a,
        Action::Presented {
            commit: CommitId(2),
            ..
        }
    )));
}

#[test]
fn held_before_vblank_callbacks_fire_after_power_on() {
    let lead = ms(4);
    let cfg = TimingConfig::default().with_callbacks(CallbackPolicy::BeforeVblank { lead });
    let mut s = sched(cfg);
    s.submit(START, Update::new(S, CommitId(1)));
    s.advance(t60(1));
    s.set_enabled(false, t60(1) + ms(1));
    let on = START + ms(300);
    s.set_enabled(true, on);
    assert_eq!(s.next_wakeup(), Some(on + P60 - lead));
}

#[test]
fn removed_surface_discards_and_releases_everything() {
    let mut s = sched(TimingConfig::default());
    s.submit(START, Update::new(S, CommitId(1)));
    s.advance(t60(1));
    s.submit(t60(1), Update::new(S, CommitId(2)).pending());
    s.submit(t60(1), Update::new(S, CommitId(3)));
    let out = s.remove_surface(S);
    assert_eq!(
        out,
        vec![
            Action::Discarded {
                surface: S,
                commit: CommitId(2)
            },
            Action::Discarded {
                surface: S,
                commit: CommitId(3)
            },
            Action::Released {
                surface: S,
                commit: CommitId(1)
            },
            Action::Released {
                surface: S,
                commit: CommitId(2)
            },
            Action::Released {
                surface: S,
                commit: CommitId(3)
            },
        ]
    );
    assert_eq!(s.unfinished(S), 0);
    assert!(
        !s.advance(t60(3))
            .iter()
            .any(|a| matches!(a, Action::Presented { .. } | Action::FrameCallbacks { .. }))
    );
}

fn us_(v: i64) -> Nanos {
    Nanos::from_micros(v)
}
