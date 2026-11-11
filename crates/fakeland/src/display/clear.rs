//! Blocking work between display deadlines. Readbacks and composites run on the compositor
//! thread, so work that ends past the next output wake makes that wake late. Such work starts
//! just after the next output wake instead.

use fakeland_types::{Nanos, Timestamp};

use crate::server::State;

/// The gap between an output wake and the work that waits for it.
const AFTER_WAKE: Nanos = Nanos::from_micros(100);

/// How often one piece of work can wait for a wake before it runs at once.
pub const MAX_WAITS: u32 = 2;

impl State {
    /// Returns the time to start work of an expected cost, or `None` to start it now. The work
    /// waits when the next output wake comes before the work would end.
    pub fn clear_slot(&self, cost: Nanos) -> Option<Timestamp> {
        let next = self
            .outputs
            .iter()
            .filter_map(|o| o.scheduler.next_wakeup())
            .min()?;
        slot(self.now(), next, cost)
    }

    /// Runs blocking work now, or after the next output wake when the work would end past it.
    /// The work waits at most [`MAX_WAITS`] times.
    pub fn run_when_clear(&mut self, cost: Nanos, task: impl FnOnce(&mut State) + 'static) {
        run_after(self, cost, 0, Box::new(task));
    }
}

fn run_after(state: &mut State, cost: Nanos, waits: u32, task: Box<dyn FnOnce(&mut State)>) {
    match state.clear_slot(cost).filter(|_| waits < MAX_WAITS) {
        Some(at) => {
            state.schedule_at(at, move |state| run_after(state, cost, waits + 1, task));
        }
        None => task(state),
    }
}

/// Returns the time after `next` when work of `cost` started at `now` ends past `next`.
fn slot(now: Timestamp, next: Timestamp, cost: Nanos) -> Option<Timestamp> {
    (next.since(now) < cost).then(|| next.max(now) + AFTER_WAKE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_waits_for_a_close_wake_only() {
        let now = Timestamp(1_000_000_000);
        let cost = Nanos::from_millis(2);
        assert_eq!(slot(now, now + Nanos::from_millis(5), cost), None);
        assert_eq!(
            slot(now, now + Nanos::from_millis(1), cost),
            Some(now + Nanos::from_millis(1) + AFTER_WAKE)
        );
        assert_eq!(
            slot(now, now - Nanos::from_millis(1), cost),
            Some(now + AFTER_WAKE)
        );
    }
}
