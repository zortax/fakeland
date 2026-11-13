//! The readback queue of a probe repeat. Pixel candidates wait in commit order, so the earliest
//! commit that shows the response wins. A readback runs clear of output wakes: near a wake the
//! queue waits for the slot after it, and later candidates wait behind the queued ones. The queue
//! also waits while the sampler thread holds the GPU gate. A discarded candidate goes at once,
//! before the vblank that releases its buffer.

use std::collections::VecDeque;

use fakeland_types::{CommitId, SurfaceId, Timestamp};

use super::detect::Step;
use super::guard;
use super::hit::test_candidate;
use super::run::Shared;
use super::watch::finish_repeat;
use crate::server::State;

/// A buffer that waits for its readback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Candidate {
    /// The commit that attached the buffer.
    pub commit: CommitId,
    /// The surface.
    pub surface: SurfaceId,
    /// The record time that a detection keeps.
    pub t: Timestamp,
    /// The commit came before the input: its region becomes the baseline of its surface.
    pub baseline: bool,
}

/// The candidates of a repeat that wait for their readback, in commit order.
#[derive(Debug, Default)]
pub(super) struct ReadbackQueue {
    items: VecDeque<Candidate>,
    /// A drain waits for a later slot.
    pub waiting: bool,
}

impl ReadbackQueue {
    /// Inserts a candidate at its place in commit order.
    pub fn insert(&mut self, c: Candidate) {
        if self.contains(c.commit) {
            return;
        }
        let i = self.items.partition_point(|q| q.commit < c.commit);
        self.items.insert(i, c);
    }

    /// Takes the candidate with the oldest commit.
    pub fn pop(&mut self) -> Option<Candidate> {
        self.items.pop_front()
    }

    /// Returns true when a candidate of the commit waits.
    pub fn contains(&self, commit: CommitId) -> bool {
        self.items.iter().any(|c| c.commit == commit)
    }

    /// Returns true when no candidate waits.
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Drops every waiting candidate.
    pub fn clear(&mut self) {
        self.items.clear();
    }
}

/// Queues a candidate. The queue drains now unless a drain already waits for its slot.
pub(super) fn enqueue(state: &mut State, run: &Shared, c: Candidate) -> Step {
    let waiting = {
        let mut r = run.borrow_mut();
        let Some(rep) = r.repeat.as_mut().filter(|rep| !rep.finished) else {
            return Step::Continue;
        };
        if rep.watch.detected.is_some() {
            return Step::Continue;
        }
        rep.watch.queue.insert(c);
        rep.watch.queue.waiting
    };
    if waiting {
        return Step::Continue;
    }
    drain(state, run, 0, 0)
}

/// Tests the queued candidates in order while each readback stays clear of the next output wake
/// and of the GPU queries of the sampler. Near a wake the drain goes on after it, at most
/// [`guard::MAX_DEFERRALS`] times in a row. While the sampler holds the GPU gate the drain tries
/// again after [`guard::GATE_RETRY`], at most [`guard::MAX_GATE_WAITS`] times in a row.
pub(super) fn drain(
    state: &mut State,
    run: &Shared,
    mut deferrals: u32,
    mut gate_waits: u32,
) -> Step {
    let gate = state.gpu_gate.clone();
    loop {
        let (token, worst) = {
            let r = run.borrow();
            let Some(rep) = r.repeat.as_ref().filter(|rep| !rep.finished) else {
                return Step::Continue;
            };
            if rep.watch.queue.is_empty() {
                return Step::Continue;
            }
            (rep.token.clone(), r.readback_max)
        };
        let held = gate.try_enter();
        let later = if held.is_none() && gate_waits < guard::MAX_GATE_WAITS {
            Some((state.now() + guard::GATE_RETRY, deferrals, gate_waits + 1))
        } else if deferrals < guard::MAX_DEFERRALS {
            guard::readback_slot(state, guard::guard_for(worst))
                .map(|at| (at, deferrals + 1, gate_waits))
        } else {
            None
        };
        if let Some((at, deferrals, gate_waits)) = later {
            set_waiting(run, true);
            let run = run.clone();
            state.schedule_at(at, move |state| {
                if token.is_cancelled() {
                    return;
                }
                set_waiting(&run, false);
                if drain(state, &run, deferrals, gate_waits) == Step::Finished {
                    finish_repeat(state, &run);
                }
            });
            return Step::Continue;
        }
        let Some(c) = pop(run) else {
            return Step::Continue;
        };
        if test_candidate(state, run, c) == Step::Finished {
            return Step::Finished;
        }
        drop(held);
        deferrals = 0;
        gate_waits = 0;
    }
}

/// Tests the queued candidates up to the one of `commit` at once, in order. Nothing happens when
/// no candidate of `commit` waits.
pub(super) fn drain_through(state: &mut State, run: &Shared, commit: CommitId) -> Step {
    loop {
        let next = {
            let mut r = run.borrow_mut();
            let Some(rep) = r.repeat.as_mut().filter(|rep| !rep.finished) else {
                return Step::Continue;
            };
            if !rep.watch.queue.contains(commit) {
                return Step::Continue;
            }
            rep.watch.queue.pop()
        };
        let Some(c) = next else {
            return Step::Continue;
        };
        let step = test_candidate(state, run, c);
        if step == Step::Finished || c.commit == commit {
            return step;
        }
    }
}

fn set_waiting(run: &Shared, waiting: bool) {
    if let Some(rep) = run.borrow_mut().repeat.as_mut() {
        rep.watch.queue.waiting = waiting;
    }
}

fn pop(run: &Shared) -> Option<Candidate> {
    run.borrow_mut()
        .repeat
        .as_mut()
        .and_then(|rep| rep.watch.queue.pop())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cand(commit: u64) -> Candidate {
        Candidate {
            commit: CommitId(commit),
            surface: SurfaceId(1),
            t: Timestamp(commit),
            baseline: false,
        }
    }

    #[test]
    fn candidates_leave_in_commit_order() {
        let mut q = ReadbackQueue::default();
        q.insert(cand(5));
        q.insert(cand(2));
        q.insert(cand(9));
        q.insert(cand(5));
        assert!(q.contains(CommitId(2)));
        let order: Vec<_> = std::iter::from_fn(|| q.pop()).map(|c| c.commit.0).collect();
        assert_eq!(order, [2, 5, 9]);
        assert!(q.is_empty());
    }

    #[test]
    fn clear_drops_every_candidate() {
        let mut q = ReadbackQueue::default();
        q.insert(cand(1));
        q.insert(cand(3));
        q.clear();
        assert!(q.is_empty());
        assert!(!q.contains(CommitId(1)));
    }
}
