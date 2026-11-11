//! A stop flag that threads can wait on.

use std::sync::{Condvar, Mutex};
use std::time::Duration;

/// A flag that is set once.
#[derive(Debug, Default)]
pub struct StopFlag {
    set: Mutex<bool>,
    cv: Condvar,
}

impl StopFlag {
    /// Sets the flag and wakes every waiter.
    pub fn set(&self) {
        *self.set.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.cv.notify_all();
    }

    /// Returns true once the flag is set.
    pub fn is_set(&self) -> bool {
        *self.set.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Blocks until the flag is set.
    pub fn wait(&self) {
        let mut set = self.set.lock().unwrap_or_else(|e| e.into_inner());
        while !*set {
            set = self.cv.wait(set).unwrap_or_else(|e| e.into_inner());
        }
    }

    /// Blocks until the flag is set or the limit passes. Returns true when the flag is set.
    pub fn wait_for(&self, limit: Duration) -> bool {
        let set = self.set.lock().unwrap_or_else(|e| e.into_inner());
        let (set, _) = self
            .cv
            .wait_timeout_while(set, limit, |s| !*s)
            .unwrap_or_else(|e| e.into_inner());
        *set
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn wakes_waiters() {
        let flag = Arc::new(StopFlag::default());
        assert!(!flag.wait_for(Duration::from_millis(5)));
        let f = Arc::clone(&flag);
        let t = std::thread::spawn(move || f.wait());
        flag.set();
        t.join().unwrap();
        assert!(flag.is_set());
        assert!(flag.wait_for(Duration::from_millis(1)));
    }
}
