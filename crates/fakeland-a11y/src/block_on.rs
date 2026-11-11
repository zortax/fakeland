//! A minimal executor for the blocking wrappers. The D-Bus connection runs its own I/O thread, so
//! a future only needs a thread that parks until its waker fires.

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

/// Runs a future to completion on the calling thread.
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
    use std::sync::Mutex;
    use std::time::Duration;

    use super::*;

    struct Flag(Arc<Mutex<(bool, Option<Waker>)>>);

    impl Future for Flag {
        type Output = u32;

        fn poll(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<u32> {
            let mut g = self.0.lock().unwrap();
            if g.0 {
                Poll::Ready(7)
            } else {
                g.1 = Some(cx.waker().clone());
                Poll::Pending
            }
        }
    }

    #[test]
    fn runs_a_future_woken_from_another_thread() {
        let shared = Arc::new(Mutex::new((false, None::<Waker>)));
        let remote = shared.clone();
        let t = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            let mut g = remote.lock().unwrap();
            g.0 = true;
            if let Some(w) = g.1.take() {
                w.wake();
            }
        });
        assert_eq!(block_on(Flag(shared)), 7);
        t.join().unwrap();
        assert_eq!(block_on(async { 3 }), 3);
    }
}
