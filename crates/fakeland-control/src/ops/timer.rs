//! A runtime-agnostic timeout for futures. A helper thread waits for the limit or for the end
//! of the future, whichever comes first; the future that finishes first wins.

use std::future::Future;
use std::sync::mpsc;

use fakeland_types::Nanos;
use futures_channel::oneshot;
use futures_util::future::{Either, select};

/// Runs a future with a time limit. Returns `None` when the limit passes first.
pub async fn with_timeout<F: Future + Unpin>(future: F, limit: Nanos) -> Option<F::Output> {
    let (fire, fired) = oneshot::channel::<()>();
    let (done, wait) = mpsc::channel::<()>();
    let limit = limit.max(Nanos::ZERO).to_duration();
    std::thread::Builder::new()
        .name("fakeland-timeout".into())
        .spawn(move || {
            if let Err(mpsc::RecvTimeoutError::Timeout) = wait.recv_timeout(limit) {
                let _ = fire.send(());
            }
        })
        .ok()?;
    let out = match select(future, fired).await {
        Either::Left((out, _)) => Some(out),
        Either::Right(_) => None,
    };
    drop(done);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finishes_or_times_out() {
        let ready =
            fakeland::block_on(with_timeout(Box::pin(async { 7 }), Nanos::from_millis(500)));
        assert_eq!(ready, Some(7));
        let never = fakeland::block_on(with_timeout(
            Box::pin(std::future::pending::<()>()),
            Nanos::from_millis(20),
        ));
        assert_eq!(never, None);
    }
}
