//! A minimal executor that runs one future to completion on the calling thread. The blocking
//! variants of the session API use it, so the crate needs no async runtime.

use std::future::Future;
use std::pin::pin;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread::{self, Thread};

struct ThreadWaker(Thread);

impl Wake for ThreadWaker {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.unpark();
    }
}

/// Polls a future on the calling thread and parks between polls.
pub fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
    let mut cx = Context::from_waker(&waker);
    loop {
        if let Poll::Ready(out) = future.as_mut().poll(&mut cx) {
            return out;
        }
        thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_a_future_woken_from_another_thread() {
        let (tx, rx) = futures_channel::oneshot::channel();
        let handle = thread::spawn(move || {
            thread::sleep(std::time::Duration::from_millis(10));
            tx.send(42).unwrap();
        });
        assert_eq!(block_on(rx), Ok(42));
        handle.join().unwrap();
    }

    #[test]
    fn ready_future_returns_at_once() {
        assert_eq!(block_on(async { 7 }), 7);
    }
}
