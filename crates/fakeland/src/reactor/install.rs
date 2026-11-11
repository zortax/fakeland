//! Installed reactors on the compositor thread. Each reactor has an observer that feeds it every
//! record; a callback takes the reactor out of its slot, so it may call anything on the state.

use std::collections::HashMap;

use fakeland_trace::Record;
use futures_channel::mpsc::UnboundedSender;

use super::{Reactor, ReactorCx, ReactorId};
use crate::framecap::CapturedFrame;
use crate::server::State;
use crate::waits::{Observe, ObserverId};

/// One callback into a reactor.
enum Call<'a> {
    Start,
    Record(&'a Record),
    Capture(CapturedFrame),
    Timer(u64),
}

trait Installed {
    /// Runs one callback. Returns false when the reactor finished.
    fn call(&mut self, state: &mut State, id: ReactorId, call: Call<'_>) -> bool;
}

struct Slot<R: Reactor> {
    reactor: R,
    tx: UnboundedSender<R::Value>,
}

impl<R: Reactor> Installed for Slot<R> {
    fn call(&mut self, state: &mut State, id: ReactorId, call: Call<'_>) -> bool {
        let mut finished = false;
        let mut cx = ReactorCx {
            state,
            id,
            tx: &self.tx,
            finished: &mut finished,
        };
        match call {
            Call::Start => self.reactor.on_start(&mut cx),
            Call::Record(r) => self.reactor.on_record(r, &mut cx),
            Call::Capture(f) => self.reactor.on_capture(f, &mut cx),
            Call::Timer(t) => self.reactor.on_timer(t, &mut cx),
        }
        !finished
    }
}

struct Entry {
    slot: Option<Box<dyn Installed>>,
    observer: Option<ObserverId>,
}

/// Every installed reactor.
#[derive(Default)]
pub struct Reactors {
    list: HashMap<ReactorId, Entry>,
    next: u64,
}

impl std::fmt::Debug for Reactors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reactors")
            .field("len", &self.list.len())
            .finish()
    }
}

impl State {
    /// Installs a reactor that sends its values to `tx`.
    pub fn install_reactor<R: Reactor>(
        &mut self,
        reactor: R,
        tx: UnboundedSender<R::Value>,
    ) -> ReactorId {
        self.reactors.next += 1;
        let id = ReactorId(self.reactors.next);
        self.reactors.list.insert(
            id,
            Entry {
                slot: Some(Box::new(Slot { reactor, tx })),
                observer: None,
            },
        );
        let observer = self.observe(move |state, record| {
            if state.call_reactor(id, Call::Record(record)) {
                Observe::Keep
            } else {
                Observe::Remove
            }
        });
        if let Some(e) = self.reactors.list.get_mut(&id) {
            e.observer = Some(observer);
        }
        self.call_reactor(id, Call::Start);
        id
    }

    /// Removes a reactor. Its handle drains and ends.
    pub fn remove_reactor(&mut self, id: ReactorId) {
        if let Some(e) = self.reactors.list.remove(&id)
            && let Some(o) = e.observer
        {
            self.unobserve(o);
        }
    }

    /// Delivers a captured frame to a reactor.
    pub(crate) fn reactor_capture(&mut self, id: ReactorId, frame: CapturedFrame) {
        self.call_reactor(id, Call::Capture(frame));
    }

    /// Delivers a timer to a reactor.
    pub(crate) fn reactor_timer(&mut self, id: ReactorId, token: u64) {
        self.call_reactor(id, Call::Timer(token));
    }

    /// Runs one callback. Returns false when the reactor is gone.
    fn call_reactor(&mut self, id: ReactorId, call: Call<'_>) -> bool {
        let Some(mut slot) = self.reactors.list.get_mut(&id).and_then(|e| e.slot.take()) else {
            return self.reactors.list.contains_key(&id);
        };
        let keep = slot.call(self, id, call);
        match self.reactors.list.get_mut(&id) {
            Some(e) if keep => {
                e.slot = Some(slot);
                true
            }
            Some(_) => {
                self.remove_reactor(id);
                false
            }
            None => false,
        }
    }
}
