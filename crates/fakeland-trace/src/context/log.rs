//! A log of context events by record index, with queries for the live context at an index.

use std::collections::{HashMap, HashSet};

use crate::event::{ContextKey, Record};

/// Context events with their record indices.
///
/// Indices grow with each observed record. The log answers which context records are live at an
/// index: records of entities that have not ended, and only the latest record of a kind that
/// replaces its earlier state.
#[derive(Clone, Debug, Default)]
pub struct ContextLog {
    entries: Vec<(u64, Record)>,
    ended: HashMap<ContextKey, u64>,
}

impl ContextLog {
    /// Builds an empty log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a context event at an index. Other events have no effect.
    pub fn observe(&mut self, index: u64, record: &Record) {
        let Some(key) = record.event.context_key() else {
            return;
        };
        if record.event.ends_context() {
            self.ended.insert(key, index);
        } else if !record.event.replaces_context() {
            // A key that starts a new life (for example a reused pid) is live again.
            self.ended.remove(&key);
        }
        self.entries.push((index, record.clone()));
    }

    /// Returns the number of held entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true when the log holds no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the context records that are live just before `index`, in record order.
    pub fn live_before(&self, index: u64) -> Vec<Record> {
        let mut ended: HashMap<ContextKey, u64> = HashMap::new();
        let mut last_replacing: HashMap<ContextKey, usize> = HashMap::new();
        for (pos, (i, r)) in self.entries.iter().enumerate() {
            if *i >= index {
                break;
            }
            let Some(key) = r.event.context_key() else {
                continue;
            };
            if r.event.ends_context() {
                ended.insert(key, *i);
            } else if r.event.replaces_context() {
                last_replacing.insert(key, pos);
            } else {
                ended.remove(&key);
            }
        }
        let mut out = Vec::new();
        for (pos, (i, r)) in self.entries.iter().enumerate() {
            if *i >= index {
                break;
            }
            let Some(key) = r.event.context_key() else {
                continue;
            };
            if r.event.ends_context() {
                continue;
            }
            if r.event.replaces_context() {
                if last_replacing.get(&key) == Some(&pos) {
                    out.push(r.clone());
                }
                continue;
            }
            // Keep the record when the entity has not ended after this record.
            match ended.get(&key) {
                Some(end) if *end > *i => {}
                _ => out.push(r.clone()),
            }
        }
        out
    }

    /// Drops entries that no query at or after `horizon` can return: records of entities that
    /// ended before `horizon`, and replaced records of focus-like kinds.
    pub fn compact(&mut self, horizon: u64) {
        let dead: HashSet<ContextKey> = self
            .ended
            .iter()
            .filter(|(_, end)| **end < horizon)
            .map(|(k, _)| *k)
            .collect();
        let mut last_replacing: HashMap<ContextKey, u64> = HashMap::new();
        for (i, r) in &self.entries {
            if *i < horizon
                && r.event.replaces_context()
                && let Some(key) = r.event.context_key()
            {
                last_replacing.insert(key, *i);
            }
        }
        self.entries.retain(|(i, r)| {
            let Some(key) = r.event.context_key() else {
                return false;
            };
            if *i < horizon && dead.contains(&key) {
                return false;
            }
            if *i < horizon && r.event.replaces_context() {
                return last_replacing.get(&key) == Some(i);
            }
            true
        });
        self.ended.retain(|k, _| !dead.contains(k));
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::{ClientId, SurfaceId, Timestamp};

    use super::*;
    use crate::event::Event;

    fn rec(event: Event) -> Record {
        Record::new(Timestamp(0), event)
    }

    fn created(s: u64) -> Record {
        rec(Event::SurfaceCreated {
            surface: SurfaceId(s),
            client: ClientId(1),
        })
    }

    fn destroyed(s: u64) -> Record {
        rec(Event::SurfaceDestroyed {
            surface: SurfaceId(s),
        })
    }

    fn focus(s: u64) -> Record {
        rec(Event::Focus {
            surface: Some(SurfaceId(s)),
        })
    }

    #[test]
    fn ended_entities_drop_out() {
        let mut log = ContextLog::new();
        log.observe(0, &created(1));
        log.observe(1, &created(2));
        log.observe(2, &destroyed(1));
        assert_eq!(log.live_before(2).len(), 2);
        let live = log.live_before(3);
        assert_eq!(live, vec![created(2)]);
    }

    #[test]
    fn focus_keeps_the_latest() {
        let mut log = ContextLog::new();
        log.observe(0, &focus(1));
        log.observe(1, &focus(2));
        assert_eq!(log.live_before(1), vec![focus(1)]);
        assert_eq!(log.live_before(5), vec![focus(2)]);
    }

    #[test]
    fn compact_keeps_live_answers() {
        let mut log = ContextLog::new();
        log.observe(0, &created(1));
        log.observe(1, &focus(1));
        log.observe(2, &created(2));
        log.observe(3, &destroyed(1));
        log.observe(4, &focus(2));
        let before = log.live_before(10);
        log.compact(5);
        assert_eq!(log.live_before(10), before);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn other_events_are_ignored() {
        let mut log = ContextLog::new();
        log.observe(0, &rec(Event::UserMark { label: "x".into() }));
        assert!(log.is_empty());
    }
}
