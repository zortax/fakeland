//! A deadline queue of scheduled tasks. Tasks with equal deadlines run in insertion order, and a
//! cancelled task never runs.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use fakeland_types::Timestamp;

/// The handle of a scheduled task.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TimerId(pub u64);

/// Tasks ordered by deadline.
pub struct TimerQueue<T> {
    heap: BinaryHeap<Reverse<(Timestamp, u64)>>,
    tasks: HashMap<u64, T>,
    next: u64,
}

impl<T> Default for TimerQueue<T> {
    fn default() -> Self {
        Self {
            heap: BinaryHeap::new(),
            tasks: HashMap::new(),
            next: 0,
        }
    }
}

impl<T> std::fmt::Debug for TimerQueue<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TimerQueue")
            .field("len", &self.tasks.len())
            .finish()
    }
}

impl<T> TimerQueue<T> {
    /// Schedules a task at a deadline.
    pub fn schedule(&mut self, at: Timestamp, task: T) -> TimerId {
        let id = self.next;
        self.next += 1;
        self.heap.push(Reverse((at, id)));
        self.tasks.insert(id, task);
        TimerId(id)
    }

    /// Cancels a task. Returns the task when it was still queued.
    pub fn cancel(&mut self, id: TimerId) -> Option<T> {
        self.tasks.remove(&id.0)
    }

    /// Returns the earliest deadline of a live task. Cancelled heads leave the heap here.
    pub fn next_deadline(&mut self) -> Option<Timestamp> {
        while let Some(Reverse((t, id))) = self.heap.peek().copied() {
            if self.tasks.contains_key(&id) {
                return Some(t);
            }
            self.heap.pop();
        }
        None
    }

    /// Removes and returns the earliest task whose deadline is at or before `now`.
    pub fn pop_due(&mut self, now: Timestamp) -> Option<(Timestamp, T)> {
        while let Some(Reverse((t, id))) = self.heap.peek().copied() {
            if t > now {
                return None;
            }
            self.heap.pop();
            if let Some(task) = self.tasks.remove(&id) {
                return Some((t, task));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pops_in_deadline_then_insertion_order() {
        let mut q = TimerQueue::default();
        q.schedule(Timestamp(30), "c");
        q.schedule(Timestamp(10), "a");
        q.schedule(Timestamp(10), "b");
        assert_eq!(q.next_deadline(), Some(Timestamp(10)));
        assert_eq!(q.pop_due(Timestamp(5)), None);
        assert_eq!(q.pop_due(Timestamp(20)), Some((Timestamp(10), "a")));
        assert_eq!(q.pop_due(Timestamp(20)), Some((Timestamp(10), "b")));
        assert_eq!(q.pop_due(Timestamp(20)), None);
        assert_eq!(q.pop_due(Timestamp(30)), Some((Timestamp(30), "c")));
        assert_eq!(q.next_deadline(), None);
    }

    #[test]
    fn cancelled_tasks_never_run() {
        let mut q = TimerQueue::default();
        let a = q.schedule(Timestamp(10), 1);
        q.schedule(Timestamp(20), 2);
        assert_eq!(q.cancel(a), Some(1));
        assert_eq!(q.cancel(a), None);
        assert_eq!(q.next_deadline(), Some(Timestamp(20)));
        assert_eq!(q.pop_due(Timestamp(100)), Some((Timestamp(20), 2)));
        assert_eq!(q.pop_due(Timestamp(100)), None);
    }
}
