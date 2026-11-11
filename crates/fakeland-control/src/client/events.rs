//! Routing of events to subscriptions and run listeners. Events of a subscription that the
//! client has not registered yet wait in a bounded buffer.

use std::collections::HashMap;

use futures_channel::mpsc::UnboundedSender;

use super::SubEvent;
use crate::methods::RunSummary;
use crate::wire::EventBody;

/// Events kept for one unregistered subscription.
const EARLY_LIMIT: usize = 4096;

/// The event routes of a connection.
#[derive(Debug, Default)]
pub struct Routes {
    subs: HashMap<u64, UnboundedSender<SubEvent>>,
    early: HashMap<u64, Vec<SubEvent>>,
    closed: Vec<u64>,
    runs: Vec<UnboundedSender<RunSummary>>,
}

impl Routes {
    /// Registers a subscription and delivers its early events.
    pub fn add_subscription(&mut self, id: u64, tx: UnboundedSender<SubEvent>) {
        for e in self.early.remove(&id).unwrap_or_default() {
            let _ = tx.unbounded_send(e);
        }
        if self.closed.contains(&id) {
            self.closed.retain(|c| *c != id);
            return;
        }
        self.subs.insert(id, tx);
    }

    /// Removes a subscription.
    pub fn remove_subscription(&mut self, id: u64) {
        self.subs.remove(&id);
        self.early.remove(&id);
    }

    /// Adds a listener for finished runs.
    pub fn add_run_listener(&mut self, tx: UnboundedSender<RunSummary>) {
        self.runs.push(tx);
    }

    /// Routes one event.
    pub fn route(&mut self, body: EventBody) {
        let (id, event) = match body {
            EventBody::Record {
                subscription,
                record,
            } => (subscription, SubEvent::Record(record)),
            EventBody::Gap {
                subscription,
                missed,
            } => (subscription, SubEvent::Gap { missed }),
            EventBody::Closed { subscription } => {
                if self.subs.remove(&subscription).is_none() {
                    self.closed.push(subscription);
                }
                return;
            }
            EventBody::RunFinished { summary } => {
                self.runs
                    .retain(|tx| tx.unbounded_send((*summary).clone()).is_ok());
                return;
            }
        };
        match self.subs.get(&id) {
            Some(tx) => {
                if tx.unbounded_send(event).is_err() {
                    self.subs.remove(&id);
                }
            }
            None => {
                let early = self.early.entry(id).or_default();
                if early.len() < EARLY_LIMIT {
                    early.push(event);
                }
            }
        }
    }

    /// Ends every route.
    pub fn close(&mut self) {
        self.subs.clear();
        self.early.clear();
        self.runs.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fakeland_trace::{Event, Record};
    use fakeland_types::Timestamp;
    use futures_channel::mpsc::{TryRecvError, unbounded};

    fn rec(sub: u64) -> EventBody {
        EventBody::Record {
            subscription: sub,
            record: Record::new(Timestamp(1), Event::UserMark { label: "x".into() }),
        }
    }

    #[test]
    fn keeps_early_events_until_registration() {
        let mut routes = Routes::default();
        routes.route(rec(5));
        routes.route(EventBody::Gap {
            subscription: 5,
            missed: 2,
        });
        let (tx, mut rx) = unbounded();
        routes.add_subscription(5, tx);
        routes.route(rec(5));
        assert!(matches!(rx.try_recv(), Ok(SubEvent::Record(_))));
        assert_eq!(rx.try_recv().ok(), Some(SubEvent::Gap { missed: 2 }));
        assert!(matches!(rx.try_recv(), Ok(SubEvent::Record(_))));
        routes.route(EventBody::Closed { subscription: 5 });
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Closed)));
    }

    #[test]
    fn closes_a_subscription_that_ended_early() {
        let mut routes = Routes::default();
        routes.route(rec(9));
        routes.route(EventBody::Closed { subscription: 9 });
        let (tx, mut rx) = unbounded();
        routes.add_subscription(9, tx);
        assert!(matches!(rx.try_recv(), Ok(SubEvent::Record(_))));
        assert!(matches!(rx.try_recv(), Err(TryRecvError::Closed)));
    }
}
