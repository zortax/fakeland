//! Cancellation of compositor-side work. Timers, observers and waiters of one task share a token;
//! once the token is cancelled, their callbacks stop and report nothing.

use std::cell::Cell;
use std::rc::Rc;

/// A shared cancellation flag on the compositor thread.
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Rc<Cell<bool>>);

impl CancelToken {
    /// Builds a live token.
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancels every holder of the token.
    pub fn cancel(&self) {
        self.0.set(true);
    }

    /// Returns true after `cancel`.
    pub fn is_cancelled(&self) -> bool {
        self.0.get()
    }
}

/// A value that one of several callbacks takes, once.
pub struct TakeOnce<T>(Rc<Cell<Option<T>>>);

impl<T> std::fmt::Debug for TakeOnce<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TakeOnce")
            .field("pending", &self.is_pending())
            .finish()
    }
}

impl<T> Clone for TakeOnce<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> TakeOnce<T> {
    /// Stores a value.
    pub fn new(value: T) -> Self {
        Self(Rc::new(Cell::new(Some(value))))
    }

    /// Takes the value. Later calls return `None`.
    pub fn take(&self) -> Option<T> {
        self.0.take()
    }

    /// Returns true while the value is there.
    pub fn is_pending(&self) -> bool {
        let v = self.0.take();
        let pending = v.is_some();
        self.0.set(v);
        pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_and_take_once() {
        let t = CancelToken::new();
        let u = t.clone();
        assert!(!u.is_cancelled());
        t.cancel();
        assert!(u.is_cancelled());
        let once = TakeOnce::new(5);
        let other = once.clone();
        assert!(other.is_pending());
        assert_eq!(once.take(), Some(5));
        assert!(!other.is_pending());
        assert_eq!(other.take(), None);
    }
}
