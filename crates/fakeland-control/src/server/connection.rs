//! One client connection: the reader loop, the writer thread, the per-request threads and the
//! methods that need connection state (subscriptions, run notification, shutdown).

use std::collections::BTreeSet;
use std::io::BufReader;
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;

use super::dispatch::{dispatch, encode, parse};
use super::stop::StopFlag;
use super::subscriptions::{Subscriptions, forward};
use super::writer::{Outbox, spawn_writer};
use crate::methods::{
    Method, RunScenario, RunScenarioParams, Shutdown, Subscribe, SubscribeParams, SubscriptionInfo,
    Unsubscribe, UnsubscribeParams,
};
use crate::ops::Controller;
use crate::wire::{ErrorBody, ErrorCode, Event, EventBody, LineReader, Request, Response};

/// The shared state of one connection.
#[derive(Debug, Clone)]
struct Conn {
    controller: Controller,
    out: Outbox,
    subs: Subscriptions,
    stop: Arc<StopFlag>,
}

/// Serves one connection until the client closes it.
pub fn serve_connection(stream: UnixStream, controller: Controller, stop: Arc<StopFlag>) {
    let read = match stream.try_clone() {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("cannot clone a control stream: {e}");
            return;
        }
    };
    let out = match spawn_writer(stream) {
        Ok(out) => out,
        Err(e) => {
            tracing::warn!("cannot start a control writer: {e}");
            return;
        }
    };
    let conn = Conn {
        controller,
        out,
        subs: Subscriptions::default(),
        stop,
    };
    let mut lines = LineReader::new(BufReader::new(read));
    loop {
        let line = match lines.next_line() {
            Ok(Some(line)) => line,
            Ok(None) => break,
            Err(e) => {
                conn.out.send(&Response::error(
                    0,
                    ErrorBody::new(ErrorCode::InvalidRequest, e.to_string()),
                ));
                break;
            }
        };
        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let id = serde_json::from_str::<Value>(&line)
                    .ok()
                    .and_then(|v| v.get("id").and_then(Value::as_u64))
                    .unwrap_or(0);
                conn.out.send(&Response::error(
                    id,
                    ErrorBody::new(ErrorCode::InvalidRequest, format!("not a request: {e}")),
                ));
                continue;
            }
        };
        let worker = conn.clone();
        let spawned = std::thread::Builder::new()
            .name(format!("fakeland-ctl-{}", request.method))
            .spawn(move || worker.handle(request));
        if let Err(e) = spawned {
            tracing::warn!("cannot start a request thread: {e}");
        }
    }
    conn.subs.clear();
}

impl Conn {
    fn handle(self, request: Request) {
        let Request { id, method, params } = request;
        let result = match method.as_str() {
            Subscribe::NAME => return self.subscribe(id, params),
            Shutdown::NAME => return self.shutdown(id),
            Unsubscribe::NAME => parse::<UnsubscribeParams>(params)
                .and_then(|p| encode(self.subs.remove(p.subscription))),
            RunScenario::NAME => self.run_scenario(params),
            _ => fakeland::block_on(dispatch(&self.controller, &method, params)),
        };
        self.out.send(&match result {
            Ok(v) => Response::ok(id, v),
            Err(e) => Response::error(id, e),
        });
    }

    fn run_scenario(&self, params: Value) -> Result<Value, ErrorBody> {
        let p: RunScenarioParams = parse(params)?;
        let out = self.out.clone();
        let notify = Box::new(move |summary: &crate::methods::RunSummary| {
            out.send(&Event::new(EventBody::RunFinished {
                summary: Box::new(summary.clone()),
            }));
        });
        let started = fakeland::block_on(self.controller.run_scenario(p, Some(notify)))
            .map_err(|e| e.to_body())?;
        encode(started)
    }

    fn subscribe(self, id: u64, params: Value) {
        let started = parse::<SubscribeParams>(params).and_then(|p| {
            let capacity = p.capacity.max(1);
            let sub = fakeland::block_on(self.controller.session().subscribe(capacity))
                .map_err(|e| crate::ops::OpError::from(e).to_body())?;
            let (sid, stop) = self.subs.register();
            Ok((sid, sub, stop, p.types.into_iter().collect::<BTreeSet<_>>()))
        });
        match started {
            Ok((sid, sub, stop, types)) => {
                self.out.send(&Response::ok(
                    id,
                    serde_json::json!(SubscriptionInfo { subscription: sid }),
                ));
                if let Err(e) = forward(sid, sub, types, stop, self.out.clone(), self.subs.clone())
                {
                    tracing::warn!("cannot start a subscription thread: {e}");
                    self.subs.remove(sid);
                }
            }
            Err(e) => {
                self.out.send(&Response::error(id, e));
            }
        }
    }

    fn shutdown(self, id: u64) {
        let result = fakeland::block_on(self.controller.shutdown(Default::default()));
        self.out.send(&match result {
            Ok(()) => Response::ok(id, Value::Null),
            Err(e) => Response::error(id, e.to_body()),
        });
        self.out.flush(Duration::from_secs(1));
        self.stop.set();
    }
}
