//! Control of a running fakeland session over a unix socket: the wire format, the typed methods,
//! the server, the client, and the operations that both the socket and the MCP server use.
//!
//! # Timing stays in the compositor
//!
//! The socket carries requests, results and events, never time measurement. A client never
//! times anything by itself: waits, scenarios, triggers and probes run on the compositor thread,
//! and every timestamp in a result or an event is a `CLOCK_MONOTONIC` compositor timestamp. A
//! socket round trip takes some hundred microseconds and varies; nothing in a result depends on
//! it. To react to a frame with input at a known time, send a scenario with triggers or anchors
//! ([`methods::RunScenario`], [`methods::InstallTriggers`]) and read the compositor timestamps
//! of the run summary.
//!
//! # Wire format
//!
//! Each message is one JSON object on one line. The socket lives at
//! `$XDG_RUNTIME_DIR/fakeland/<wayland socket>.sock` by default, in a directory with mode 0700,
//! and has mode 0600.
//!
//! - A request is `{"id": 1, "method": "windows", "params": {...}}`. Missing params read as `{}`.
//! - A response is `{"id": 1, "result": ...}` or
//!   `{"id": 1, "error": {"code": "timeout", "message": "..."}}`.
//! - An event is `{"event": {"type": "record", "subscription": 3, "record": {...}}}`. The other
//!   event types are `gap` (the subscription dropped records), `closed` (the subscription
//!   ended) and `run_finished` (a run that this connection started has ended).
//!
//! Requests run in parallel, and responses can arrive out of order. [`methods`] lists every
//! method with its parameter and result types.
//!
//! # Use
//!
//! [`server::serve_session`] serves a session on a background thread. [`ControlClient`] connects
//! and calls methods; [`ControlClient::call`] takes a method marker type such as
//! [`methods::Windows`] and returns its typed result.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod client;
pub mod export;
pub mod methods;
pub mod ops;
pub mod server;
pub mod socket;
pub mod spec;
pub mod text;
pub mod wire;

pub use client::{ClientError, ClientSubscription, ControlClient, SubEvent};
pub use ops::{Controller, OpError};
pub use server::{ControlServer, ServeError, serve, serve_session};
pub use spec::{SessionSpec, SpecError};
