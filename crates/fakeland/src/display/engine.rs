//! The timing engine. One deadline timer serves every output scheduler and the queue of scheduled
//! tasks. A wake runs everything that is due in deadline order, then the timer arms at the next
//! deadline. With a timer spin the timer fires early and the thread spins to the deadline. Each
//! output wake adds its lateness to the run validity.

use fakeland_timing::Action;
use fakeland_trace::Event;
use fakeland_types::{Nanos, OutputId, Timestamp};

use super::{TimerId, TimerQueue};
use crate::server::State;

/// A task that runs on the compositor thread at its deadline.
pub type Task = Box<dyn FnOnce(&mut State)>;

/// The upper bound of due items one wake runs before the loop gets a turn.
const MAX_ITEMS_PER_WAKE: usize = 4096;

/// The queue of scheduled tasks.
#[derive(Default)]
pub struct Timers {
    queue: TimerQueue<Task>,
}

impl std::fmt::Debug for Timers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.queue.fmt(f)
    }
}

impl State {
    /// Runs `task` on the compositor thread at `at`. A deadline in the past runs at the next wake.
    pub fn schedule_at(
        &mut self,
        at: Timestamp,
        task: impl FnOnce(&mut State) + 'static,
    ) -> TimerId {
        self.timers.queue.schedule(at, Box::new(task))
    }

    /// Runs `task` after a span from now.
    pub fn schedule_in(
        &mut self,
        after: Nanos,
        task: impl FnOnce(&mut State) + 'static,
    ) -> TimerId {
        let at = self.now() + after.max(Nanos::ZERO);
        self.schedule_at(at, task)
    }

    /// Cancels a scheduled task. Returns true when it had not run.
    pub fn cancel_timer(&mut self, id: TimerId) -> bool {
        self.timers.queue.cancel(id).is_some()
    }

    /// Returns the earliest deadline of any output or task.
    pub fn next_deadline(&mut self) -> Option<Timestamp> {
        let outputs = self
            .outputs
            .iter()
            .filter_map(|o| o.scheduler.next_wakeup())
            .min();
        match (outputs, self.timers.queue.next_deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// Handles a timer wake: spins to a close deadline, then runs every due item.
    pub fn on_timer(&mut self) {
        self.timer.fired();
        let spin = self.config.timer_spin;
        if spin > Nanos::ZERO
            && let Some(next) = self.next_deadline()
        {
            let now = self.now();
            if next > now && next.since(now) <= spin {
                while self.now() < next {
                    std::hint::spin_loop();
                }
            }
        }
        self.run_due();
    }

    /// Runs every output deadline and task that is due, earliest first.
    pub fn run_due(&mut self) {
        for _ in 0..MAX_ITEMS_PER_WAKE {
            let now = self.now();
            let output = self
                .outputs
                .iter()
                .filter_map(|o| o.scheduler.next_wakeup().map(|t| (t, o.id)))
                .min();
            let task = self.timers.queue.next_deadline();
            match (output, task) {
                (Some((t, id)), task) if t <= now && task.is_none_or(|k| t <= k) => {
                    let actions = match self.outputs.get_mut(id) {
                        Some(o) => o.scheduler.advance(now),
                        None => continue,
                    };
                    self.record_wake(id, t, now, &actions);
                    self.execute_actions(id, actions);
                }
                (_, Some(k)) if k <= now => {
                    if let Some((_, task)) = self.timers.queue.pop_due(now) {
                        task(self);
                    }
                }
                _ => break,
            }
        }
    }

    /// Adds the lateness of one output wake to the run validity. A wake whose earliest deadline
    /// is no vblank also gets a trace record, since no `Vblank` record carries its lateness.
    fn record_wake(
        &mut self,
        output: OutputId,
        deadline: Timestamp,
        woke: Timestamp,
        actions: &[Action],
    ) {
        self.lateness.record(deadline, woke);
        let first_vblank = actions.iter().find_map(|a| match a {
            Action::Vblank { ideal, .. } => Some(*ideal),
            _ => None,
        });
        if first_vblank != Some(deadline) {
            self.emit_at(
                woke,
                Event::DeadlineWake {
                    output,
                    deadline,
                    woke,
                },
            );
        }
    }

    /// Arms the deadline timer at the next deadline, early by the spin.
    pub fn rearm_timer(&mut self) {
        let spin = self.config.timer_spin.max(Nanos::ZERO);
        let at = self.next_deadline().map(|t| {
            if spin > Nanos::ZERO {
                Timestamp(t.0.saturating_sub(spin.as_nanos() as u64))
            } else {
                t
            }
        });
        if let Err(e) = self.timer.arm(at) {
            tracing::error!("cannot arm the deadline timer: {e}");
        }
    }
}
