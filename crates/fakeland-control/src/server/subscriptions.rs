//! Subscriptions of one connection. Each subscription has a forwarding thread that filters
//! records by type and queues them as events.

use std::collections::{BTreeSet, HashMap};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use fakeland::{Subscription, SubscriptionItem};
use futures_channel::oneshot;
use futures_util::StreamExt;
use futures_util::future::{Either, select};

use super::writer::Outbox;
use crate::wire::{Event, EventBody};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// The live subscriptions of a connection. Dropping the set ends every subscription.
#[derive(Debug, Default, Clone)]
pub struct Subscriptions {
    stops: Arc<Mutex<HashMap<u64, oneshot::Sender<()>>>>,
}

impl Subscriptions {
    /// Allocates an id and registers the stop handle of a new subscription.
    pub fn register(&self) -> (u64, oneshot::Receiver<()>) {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.stops
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(id, tx);
        (id, rx)
    }

    /// Ends a subscription. Returns false for an unknown id.
    pub fn remove(&self, id: u64) -> bool {
        let stop = self
            .stops
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
        stop.map(|s| s.send(())).is_some()
    }

    /// Ends every subscription.
    pub fn clear(&self) {
        self.stops.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    fn forget(&self, id: u64) {
        self.stops
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
    }
}

/// Starts the forwarding thread of a subscription.
pub fn forward(
    id: u64,
    mut sub: Subscription,
    types: BTreeSet<String>,
    mut stop: oneshot::Receiver<()>,
    out: Outbox,
    subs: Subscriptions,
) -> std::io::Result<()> {
    std::thread::Builder::new()
        .name(format!("fakeland-ctl-sub-{id}"))
        .spawn(move || {
            loop {
                let next = fakeland::block_on(select(sub.next(), &mut stop));
                let body = match next {
                    Either::Left((Some(SubscriptionItem::Record(record)), _)) => {
                        if !types.is_empty() && !types.contains(record.event.name()) {
                            continue;
                        }
                        EventBody::Record {
                            subscription: id,
                            record,
                        }
                    }
                    Either::Left((Some(SubscriptionItem::Gap { missed }), _)) => EventBody::Gap {
                        subscription: id,
                        missed,
                    },
                    Either::Left((None, _)) | Either::Right(_) => break,
                };
                if !out.send(&Event::new(body)) {
                    break;
                }
            }
            subs.forget(id);
            out.send(&Event::new(EventBody::Closed { subscription: id }));
        })
        .map(|_| ())
}
