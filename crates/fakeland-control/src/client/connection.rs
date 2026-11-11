//! The client connection: request ids, pending replies and the reader thread.

use std::collections::HashMap;
use std::io::{BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use futures_channel::{mpsc, oneshot};
use serde_json::Value;

use super::events::Routes;
use super::{ClientError, ClientSubscription};
use crate::methods::{
    Method, RunResultOf, RunResultParams, RunScenario, RunScenarioParams, RunSummary, Subscribe,
    SubscribeParams, Unsubscribe, UnsubscribeParams,
};
use crate::socket::{SOCKET_ENV, discover_sockets};
use crate::wire::{ErrorBody, LineReader, Request, ServerMessage, encode_line};

type Reply = oneshot::Sender<Result<Value, ErrorBody>>;

/// The shared state of a client connection.
#[derive(Debug)]
pub struct Inner {
    path: PathBuf,
    writer: Mutex<UnixStream>,
    next_id: AtomicU64,
    pending: Mutex<HashMap<u64, Reply>>,
    routes: Mutex<Routes>,
    closed: AtomicBool,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

impl Inner {
    fn send_line(&self, line: &str) -> std::io::Result<()> {
        lock(&self.writer).write_all(line.as_bytes())
    }

    /// Sends an unsubscribe request without waiting for its answer.
    pub(super) fn forget_subscription(&self, id: u64) {
        lock(&self.routes).remove_subscription(id);
        let req = Request::new(
            self.next_id.fetch_add(1, Ordering::Relaxed),
            Unsubscribe::NAME,
            serde_json::json!(UnsubscribeParams { subscription: id }),
        );
        if let Ok(line) = encode_line(&req) {
            let _ = self.send_line(&line);
        }
    }

    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        let pending: Vec<Reply> = lock(&self.pending).drain().map(|(_, r)| r).collect();
        drop(pending);
        lock(&self.routes).close();
    }
}

/// A connection to a control socket. Clones share the connection.
#[derive(Debug, Clone)]
pub struct ControlClient {
    inner: Arc<Inner>,
}

impl ControlClient {
    /// Connects to a control socket.
    pub fn connect(path: impl AsRef<Path>) -> Result<Self, ClientError> {
        let path = path.as_ref().to_path_buf();
        let stream = UnixStream::connect(&path).map_err(|error| ClientError::Connect {
            path: path.clone(),
            error,
        })?;
        let read = stream.try_clone()?;
        let inner = Arc::new(Inner {
            path,
            writer: Mutex::new(stream),
            next_id: AtomicU64::new(1),
            pending: Mutex::new(HashMap::new()),
            routes: Mutex::new(Routes::default()),
            closed: AtomicBool::new(false),
        });
        let reader = Arc::clone(&inner);
        std::thread::Builder::new()
            .name("fakeland-ctl-client".into())
            .spawn(move || read_loop(read, &reader))?;
        Ok(Self { inner })
    }

    /// Connects to the socket in `FAKELAND_SOCKET`, or to the only running socket in
    /// `$XDG_RUNTIME_DIR/fakeland/`.
    pub fn connect_default() -> Result<Self, ClientError> {
        Self::connect(Self::default_path()?)
    }

    /// Returns the socket that [`ControlClient::connect_default`] picks.
    pub fn default_path() -> Result<PathBuf, ClientError> {
        if let Some(p) = std::env::var_os(SOCKET_ENV).filter(|p| !p.is_empty()) {
            return Ok(PathBuf::from(p));
        }
        let mut found = discover_sockets();
        match found.len() {
            0 => Err(ClientError::NoSocket),
            1 => Ok(found.remove(0)),
            _ => Err(ClientError::Ambiguous(found)),
        }
    }

    /// Returns the socket path.
    pub fn path(&self) -> &Path {
        &self.inner.path
    }

    /// Calls a method by name with raw JSON parameters.
    pub async fn call_raw(&self, method: &str, params: Value) -> Result<Value, ClientError> {
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(ClientError::Closed);
        }
        let id = self.inner.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        lock(&self.inner.pending).insert(id, tx);
        let line = encode_line(&Request::new(id, method, params))?;
        if let Err(e) = self.inner.send_line(&line) {
            lock(&self.inner.pending).remove(&id);
            return Err(e.into());
        }
        match rx.await {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(body)) => Err(ClientError::Remote(body)),
            Err(_) => Err(ClientError::Closed),
        }
    }

    /// Calls a method by name on the calling thread.
    pub fn blocking_call_raw(&self, method: &str, params: Value) -> Result<Value, ClientError> {
        fakeland::block_on(self.call_raw(method, params))
    }

    /// Calls a typed method.
    pub async fn call<M: Method>(&self, params: M::Params) -> Result<M::Output, ClientError> {
        let value = self
            .call_raw(M::NAME, serde_json::to_value(params)?)
            .await?;
        Ok(serde_json::from_value(value)?)
    }

    /// Calls a typed method on the calling thread.
    pub fn blocking_call<M: Method>(&self, params: M::Params) -> Result<M::Output, ClientError> {
        fakeland::block_on(self.call::<M>(params))
    }

    /// Subscribes to trace records.
    pub async fn subscribe(
        &self,
        params: SubscribeParams,
    ) -> Result<ClientSubscription, ClientError> {
        let info = self.call::<Subscribe>(params).await?;
        let (tx, rx) = mpsc::unbounded();
        lock(&self.inner.routes).add_subscription(info.subscription, tx);
        Ok(ClientSubscription::new(
            info.subscription,
            rx,
            Arc::downgrade(&self.inner),
        ))
    }

    /// Subscribes on the calling thread.
    pub fn blocking_subscribe(
        &self,
        params: SubscribeParams,
    ) -> Result<ClientSubscription, ClientError> {
        fakeland::block_on(self.subscribe(params))
    }

    /// Returns a stream of the summaries of runs that this connection starts.
    pub fn run_events(&self) -> mpsc::UnboundedReceiver<RunSummary> {
        let (tx, rx) = mpsc::unbounded();
        lock(&self.inner.routes).add_run_listener(tx);
        rx
    }

    /// Runs a scenario and waits for its summary.
    pub async fn run_and_wait(&self, params: RunScenarioParams) -> Result<RunSummary, ClientError> {
        let started = self.call::<RunScenario>(params).await?;
        loop {
            let state = self
                .call::<RunResultOf>(RunResultParams {
                    run: started.run,
                    wait: Some(fakeland_types::Dur::from_secs(60)),
                })
                .await?;
            if let Some(summary) = state.summary {
                return Ok(summary);
            }
        }
    }

    /// Runs a scenario and waits for its summary on the calling thread.
    pub fn blocking_run_and_wait(
        &self,
        params: RunScenarioParams,
    ) -> Result<RunSummary, ClientError> {
        fakeland::block_on(self.run_and_wait(params))
    }
}

fn read_loop(stream: UnixStream, inner: &Inner) {
    let mut lines = LineReader::new(BufReader::new(stream));
    while let Ok(Some(line)) = lines.next_line() {
        match serde_json::from_str::<ServerMessage>(&line) {
            Ok(ServerMessage::Response(r)) => {
                if let Some(reply) = lock(&inner.pending).remove(&r.id) {
                    let _ = reply.send(r.into_result());
                }
            }
            Ok(ServerMessage::Event(e)) => lock(&inner.routes).route(e.event),
            Err(e) => tracing::warn!("bad control message: {e}"),
        }
    }
    inner.close();
}
