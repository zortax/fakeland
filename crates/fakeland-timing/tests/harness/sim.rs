//! The simulation loop. It sleeps until the earliest scheduler deadline or client wake, moves the
//! fake clock there, lets clients commit and feeds the scheduler actions back to them.

use fakeland_timing::{Action, Scheduler, TimingConfig};
use fakeland_types::{Clock, CommitId, FakeClock, Nanos, SurfaceId, Timestamp};

use super::Client;

/// The construction time of every simulated scheduler.
pub const START: Timestamp = Timestamp(1_000_000_000);
/// The 60 Hz period.
pub const P60: Nanos = Nanos(16_666_667);

/// Returns a span in milliseconds.
pub fn ms(v: i64) -> Nanos {
    Nanos::from_millis(v)
}

/// Returns a span in microseconds.
pub fn us(v: i64) -> Nanos {
    Nanos::from_micros(v)
}

/// Returns the time of vblank `n` on the 60 Hz grid that starts at `START`.
pub fn t60(n: i64) -> Timestamp {
    START + Nanos(P60.as_nanos() * n)
}

/// A scheduler, a fake clock, synthetic clients and the log of every action.
pub struct Sim {
    pub sched: Scheduler,
    pub clock: FakeClock,
    pub log: Vec<(Timestamp, Action)>,
    clients: Vec<Box<dyn Client>>,
    next_commit: u64,
}

impl Sim {
    pub fn new(cfg: TimingConfig) -> Self {
        Self {
            sched: Scheduler::new(cfg, START).unwrap_or_else(|e| panic!("{e}")),
            clock: FakeClock::new(START),
            log: Vec::new(),
            clients: Vec::new(),
            next_commit: 1,
        }
    }

    pub fn add(&mut self, client: impl Client + 'static) {
        self.clients.push(Box::new(client));
    }

    pub fn now(&self) -> Timestamp {
        self.clock.now()
    }

    /// Runs until the next event lies after `end`, then moves the clock to `end`.
    pub fn run_until(&mut self, end: Timestamp) {
        loop {
            let client_wake = self.clients.iter().filter_map(|c| c.next_wake()).min();
            let next = [client_wake, self.sched.next_wakeup()]
                .into_iter()
                .flatten()
                .min();
            let Some(next) = next.filter(|t| *t <= end) else {
                break;
            };
            self.step_to(next.max(self.now()));
        }
        self.clock.set(end.max(self.now()));
    }

    fn step_to(&mut self, now: Timestamp) {
        self.clock.set(now);
        for i in 0..self.clients.len() {
            if self.clients[i].next_wake().is_some_and(|w| w <= now) {
                let updates = self.clients[i].wake(now, &mut self.next_commit);
                for u in updates {
                    self.sched.submit(now, u);
                }
            }
        }
        let actions = self.sched.advance(now);
        for a in actions {
            for c in &mut self.clients {
                c.observe(now, &a);
            }
            self.log.push((now, a));
        }
    }

    /// Returns the presented refreshes of a surface: sequence number and time.
    pub fn presented(&self, surface: SurfaceId) -> Vec<(u64, Timestamp)> {
        self.log
            .iter()
            .filter_map(|(_, a)| match a {
                Action::Presented {
                    surface: s,
                    seq,
                    time,
                    ..
                } if *s == surface => Some((*seq, *time)),
                _ => None,
            })
            .collect()
    }

    /// Returns the presented commits of a surface.
    pub fn presented_commits(&self, surface: SurfaceId) -> Vec<CommitId> {
        self.log
            .iter()
            .filter_map(|(_, a)| match a {
                Action::Presented {
                    surface: s, commit, ..
                } if *s == surface => Some(*commit),
                _ => None,
            })
            .collect()
    }

    /// Returns the presented sequence numbers of a surface.
    pub fn presented_seqs(&self, surface: SurfaceId) -> Vec<u64> {
        self.presented(surface)
            .into_iter()
            .map(|(s, _)| s)
            .collect()
    }

    /// Returns the times of the callbacks that reached a surface.
    pub fn callback_times(&self, surface: SurfaceId) -> Vec<Timestamp> {
        self.log
            .iter()
            .filter_map(|(_, a)| match a {
                Action::FrameCallbacks { time, callbacks }
                    if callbacks.iter().any(|c| c.surface == surface) =>
                {
                    Some(*time)
                }
                _ => None,
            })
            .collect()
    }

    /// Returns every refresh: sequence number, ideal time and the drop flag.
    pub fn vblanks(&self) -> Vec<(u64, Timestamp, bool)> {
        self.log
            .iter()
            .filter_map(|(_, a)| match a {
                Action::Vblank {
                    seq,
                    ideal,
                    dropped,
                    ..
                } => Some((*seq, *ideal, *dropped)),
                _ => None,
            })
            .collect()
    }

    /// Returns the discarded commits of a surface.
    pub fn discarded(&self, surface: SurfaceId) -> Vec<CommitId> {
        self.log
            .iter()
            .filter_map(|(_, a)| match a {
                Action::Discarded { surface: s, commit } if *s == surface => Some(*commit),
                _ => None,
            })
            .collect()
    }

    /// Returns the released commits of a surface.
    pub fn released(&self, surface: SurfaceId) -> Vec<CommitId> {
        self.log
            .iter()
            .filter_map(|(_, a)| match a {
                Action::Released { surface: s, commit } if *s == surface => Some(*commit),
                _ => None,
            })
            .collect()
    }
}
