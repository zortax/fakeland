//! The reactor driver: installs a reactor per iteration and collects its values until it
//! finishes or a timeout passes.

use std::future::Future;
use std::pin::pin;
use std::task::Poll;

use fakeland::{Reactor, Session};
use fakeland_types::Dur;
use serde::Serialize;

use super::sleep::sleep;
use super::{DriveFuture, Driver, DriverOutput, IterationContext};

/// Installs the reactor that a closure builds for each iteration. The iteration ends when the
/// reactor calls `finish`; after `timeout` the runner removes it and marks the iteration failed.
/// Every value the reactor emits lands in [`DriverOutput::values`] as JSON.
pub struct ReactorDriver<F> {
    make: F,
    timeout: Dur,
}

impl<F> ReactorDriver<F> {
    /// Builds the driver.
    pub fn new(timeout: Dur, make: F) -> Self {
        Self { make, timeout }
    }
}

impl<F, R> Driver for ReactorDriver<F>
where
    F: Fn(&IterationContext) -> R + Send + Sync,
    R: Reactor,
    R::Value: Serialize,
{
    fn drive<'a>(&'a self, session: &'a Session, cx: &'a IterationContext) -> DriveFuture<'a> {
        Box::pin(async move {
            let handle = session.install_reactor((self.make)(cx)).await?;
            let mut deadline = pin!(sleep(session, self.timeout));
            let mut out = DriverOutput::default();
            loop {
                let next = {
                    let mut recv = pin!(handle.recv());
                    std::future::poll_fn(|task| {
                        if let Poll::Ready(v) = recv.as_mut().poll(task) {
                            return Poll::Ready(Some(v));
                        }
                        match deadline.as_mut().poll(task) {
                            Poll::Ready(_) => Poll::Ready(None),
                            Poll::Pending => Poll::Pending,
                        }
                    })
                    .await
                };
                match next {
                    Some(Some(v)) => out.values.push(
                        serde_json::to_value(&v)
                            .unwrap_or_else(|e| serde_json::json!({ "error": e.to_string() })),
                    ),
                    Some(None) => break,
                    None => {
                        out.values.extend(
                            handle
                                .drain()
                                .iter()
                                .filter_map(|v| serde_json::to_value(v).ok()),
                        );
                        handle.remove().await?;
                        out.error = Some(format!("the reactor ran past {}", self.timeout));
                        break;
                    }
                }
            }
            Ok(out)
        })
    }
}
