//! Pending waits. A waiter polls after every dispatch until it reports done; at its deadline it
//! polls once more with the timeout flag. The deadline schedules a wake, so a timeout fires on
//! time even when nothing else happens.

use fakeland_types::Timestamp;

use crate::server::State;

/// A wait callback. It gets `timed_out = true` at or after its deadline and returns true when
/// the wait is over.
pub type WaitPoll = Box<dyn FnMut(&mut State, bool) -> bool>;

struct Waiter {
    deadline: Option<Timestamp>,
    poll: WaitPoll,
}

/// Every pending wait.
#[derive(Default)]
pub struct Waiters {
    list: Vec<Waiter>,
}

impl std::fmt::Debug for Waiters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Waiters")
            .field("len", &self.list.len())
            .finish()
    }
}

impl State {
    /// Adds a wait. It polls once at once.
    pub fn add_waiter(
        &mut self,
        deadline: Option<Timestamp>,
        poll: impl FnMut(&mut State, bool) -> bool + 'static,
    ) {
        let mut waiter = Waiter {
            deadline,
            poll: Box::new(poll),
        };
        let timed_out = deadline.is_some_and(|d| self.now() >= d);
        if (waiter.poll)(self, timed_out) {
            return;
        }
        if let Some(d) = deadline {
            self.schedule_at(d, |_| {});
        }
        self.waiters.list.push(waiter);
    }

    /// Polls every pending wait.
    pub fn poll_waiters(&mut self) {
        if self.waiters.list.is_empty() {
            return;
        }
        let now = self.now();
        let mut list = std::mem::take(&mut self.waiters.list);
        list.retain_mut(|w| {
            let timed_out = w.deadline.is_some_and(|d| now >= d);
            !(w.poll)(self, timed_out)
        });
        list.append(&mut self.waiters.list);
        self.waiters.list = list;
    }

    /// Ends every wait with the timeout flag, for shutdown.
    pub fn cancel_waiters(&mut self) {
        let list = std::mem::take(&mut self.waiters.list);
        for mut w in list {
            (w.poll)(self, true);
        }
    }
}
