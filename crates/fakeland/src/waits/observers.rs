//! In-thread record observers. An observer sees every trace record on the compositor thread after
//! the dispatch that produced it, with full access to the state. Triggers, reactors and probes
//! build on it.

use fakeland_trace::Record;

use crate::server::State;

/// The handle of an observer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ObserverId(pub u64);

/// Whether an observer stays installed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observe {
    /// Keep the observer.
    Keep,
    /// Remove the observer.
    Remove,
}

/// An observer callback.
pub type Observer = Box<dyn FnMut(&mut State, &Record) -> Observe>;

/// The upper bound of observer rounds per dispatch. Observers that emit records start new
/// rounds.
const MAX_ROUNDS: usize = 64;

/// Every installed observer.
#[derive(Default)]
pub struct Observers {
    list: Vec<(ObserverId, Observer)>,
    removed: Vec<ObserverId>,
    next: u64,
}

impl std::fmt::Debug for Observers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Observers")
            .field("len", &self.list.len())
            .finish()
    }
}

impl State {
    /// Installs an observer.
    pub fn observe(
        &mut self,
        f: impl FnMut(&mut State, &Record) -> Observe + 'static,
    ) -> ObserverId {
        let id = ObserverId(self.observers.next);
        self.observers.next += 1;
        self.observers.list.push((id, Box::new(f)));
        self.bus.set_observing(true);
        id
    }

    /// Removes an observer.
    pub fn unobserve(&mut self, id: ObserverId) {
        self.observers.list.retain(|(o, _)| *o != id);
        self.observers.removed.push(id);
        if self.observers.list.is_empty() {
            self.bus.set_observing(false);
        }
    }

    /// Delivers the queued records to every observer.
    pub fn run_observers(&mut self) {
        for _ in 0..MAX_ROUNDS {
            let records = self.bus.take_observed();
            if records.is_empty() {
                return;
            }
            let mut list = std::mem::take(&mut self.observers.list);
            self.observers.removed.clear();
            for record in &records {
                list.retain_mut(|(id, f)| {
                    !self.observers.removed.contains(id) && f(self, record) == Observe::Keep
                });
            }
            let removed = std::mem::take(&mut self.observers.removed);
            list.retain(|(id, _)| !removed.contains(id));
            list.append(&mut self.observers.list);
            self.observers.list = list;
            if self.observers.list.is_empty() {
                self.bus.set_observing(false);
            }
        }
    }
}
