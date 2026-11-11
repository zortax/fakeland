//! A first-in first-out queue of items that become due at a time. Items keep their insertion
//! order even when a later item has an earlier due time, so delayed inputs apply in order.

use std::collections::VecDeque;

use fakeland_types::Timestamp;

/// Items with due times, released in insertion order.
#[derive(Debug)]
pub struct DelayQueue<T> {
    items: VecDeque<(Timestamp, T)>,
}

impl<T> Default for DelayQueue<T> {
    fn default() -> Self {
        Self {
            items: VecDeque::new(),
        }
    }
}

impl<T> DelayQueue<T> {
    /// Appends an item that becomes due at `due`.
    pub fn push(&mut self, due: Timestamp, item: T) {
        self.items.push_back((due, item));
    }

    /// Removes and returns the front item when it is due at `now`.
    pub fn pop_due(&mut self, now: Timestamp) -> Option<T> {
        if self.items.front().is_some_and(|(due, _)| *due <= now) {
            self.items.pop_front().map(|(_, item)| item)
        } else {
            None
        }
    }

    /// Returns the due time of the front item.
    pub fn next_due(&self) -> Option<Timestamp> {
        self.items.front().map(|(due, _)| *due)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releases_in_order_when_due() {
        let mut q = DelayQueue::default();
        q.push(Timestamp(10), 'a');
        q.push(Timestamp(5), 'b');
        assert_eq!(q.next_due(), Some(Timestamp(10)));
        assert_eq!(q.pop_due(Timestamp(9)), None);
        assert_eq!(q.pop_due(Timestamp(10)), Some('a'));
        assert_eq!(q.pop_due(Timestamp(10)), Some('b'));
        assert_eq!(q.next_due(), None);
    }
}
