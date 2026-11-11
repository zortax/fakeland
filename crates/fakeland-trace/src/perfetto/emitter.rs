//! The export state: the time base, process ids per client, tracks, events, and the slices that
//! flow arrows bind to.

use std::collections::HashMap;

use fakeland_types::{ClientId, CommitId, ProcessId, SurfaceId, Timestamp};

use crate::event::{AppMarkKind, Event};
use crate::frames::FrameTimeline;
use crate::perfetto::chrome::ChromeEvent;
use crate::perfetto::tracks::Tracks;
use crate::trace::Trace;

/// The process id of the compositor process.
pub(crate) const COMPOSITOR_PID: u64 = 1 << 28;

/// The base of synthetic process ids for clients without a known process.
const CLIENT_PID_BASE: u64 = 1 << 29;

/// The offset in microseconds of a flow point inside the slice it binds to.
pub(crate) const FLOW_EPS: f64 = 0.01;

/// The shortest slice in microseconds. Flow points fit inside it.
pub(crate) const MIN_DUR: f64 = 1.0;

/// A slice that a flow point binds to.
#[derive(Clone, Copy)]
pub(crate) struct Slot {
    pub(crate) pid: u64,
    pub(crate) tid: u64,
    pub(crate) start: f64,
}

/// The export state.
pub(crate) struct Emitter<'a> {
    pub(crate) trace: &'a Trace,
    pub(crate) tl: &'a FrameTimeline,
    pub(crate) tracks: Tracks,
    pub(crate) events: Vec<ChromeEvent>,
    pub(crate) frame_slots: HashMap<CommitId, Slot>,
    pub(crate) screen_slots: HashMap<CommitId, Slot>,
    base: Timestamp,
}

impl<'a> Emitter<'a> {
    /// Builds the state and names the compositor process.
    pub(crate) fn new(trace: &'a Trace, tl: &'a FrameTimeline) -> Self {
        let mut tracks = Tracks::default();
        tracks.process(COMPOSITOR_PID, "compositor", -1);
        Self {
            trace,
            tl,
            tracks,
            events: Vec::new(),
            frame_slots: HashMap::new(),
            screen_slots: HashMap::new(),
            base: earliest(trace),
        }
    }

    /// Returns a timestamp in microseconds since the trace base.
    pub(crate) fn us(&self, t: Timestamp) -> f64 {
        (t - self.base).as_nanos() as f64 / 1000.0
    }

    /// Returns the end of the trace in microseconds.
    pub(crate) fn end_us(&self) -> f64 {
        self.us(self.trace.end())
    }

    /// Appends an event.
    pub(crate) fn push(&mut self, e: ChromeEvent) {
        self.events.push(e);
    }

    /// Returns the process of a client and names it on first use.
    pub(crate) fn client_pid(&mut self, client: Option<ClientId>) -> u64 {
        let info = client.and_then(|c| self.tl.client(c));
        let pid = match (info.and_then(|c| c.pid), client) {
            (Some(p), _) => u64::from(p.0),
            (None, Some(c)) => CLIENT_PID_BASE + c.0,
            (None, None) => CLIENT_PID_BASE - 1,
        };
        let name = self.client_name(client, info.and_then(|c| c.pid));
        self.tracks.process(pid, &name, pid as i64);
        pid
    }

    /// Returns the process of a surface's client.
    pub(crate) fn surface_pid(&mut self, surface: SurfaceId) -> u64 {
        let client = self.tl.surface(surface).and_then(|s| s.client);
        self.client_pid(client)
    }

    /// Returns the process of an operating system process and names it on first use.
    pub(crate) fn os_pid(&mut self, pid: ProcessId) -> u64 {
        let client = self
            .tl
            .clients()
            .find(|c| c.pid == Some(pid))
            .map(|c| c.client);
        if client.is_some() {
            return self.client_pid(client);
        }
        let name = self.client_name(None, Some(pid));
        self.tracks
            .process(u64::from(pid.0), &name, i64::from(pid.0));
        u64::from(pid.0)
    }

    /// Returns the label of a surface: its id and role.
    pub(crate) fn surface_label(&self, surface: SurfaceId) -> String {
        match self.tl.surface(surface).and_then(|s| s.role.as_deref()) {
            Some(role) => format!("{surface} {role}"),
            None => surface.to_string(),
        }
    }

    fn client_name(&self, client: Option<ClientId>, pid: Option<ProcessId>) -> String {
        let app_id = client.and_then(|c| {
            self.tl
                .windows()
                .find(|w| self.tl.surface(w.surface).and_then(|s| s.client) == Some(c))
                .map(|w| w.app_id.clone())
                .filter(|a| !a.is_empty())
        });
        let command = pid
            .and_then(|p| self.tl.process(p))
            .and_then(|p| p.command.split_whitespace().next().map(str::to_string));
        let base = app_id
            .or(command)
            .or_else(|| client.map(|c| c.to_string()))
            .unwrap_or_else(|| "unknown client".to_string());
        match pid {
            Some(p) => format!("{base} ({p})"),
            None => base,
        }
    }
}

/// Returns the earliest time the export shows: the trace start, or an earlier time that a record
/// names, such as an ideal vblank time or the begin of an app span.
fn earliest(trace: &Trace) -> Timestamp {
    let mut t = trace.header.started.min(trace.start());
    for r in &trace.records {
        let named = match &r.event {
            Event::Vblank { ideal, .. } => *ideal,
            Event::Presented { time, .. } => *time,
            Event::AppMark {
                mark: AppMarkKind::Span { begin, .. },
                ..
            } => *begin,
            Event::CompositorOverhead { duration, .. } => r.t - *duration,
            Event::Capture { cost, .. } => r.t - *cost,
            _ => r.t,
        };
        t = t.min(named).min(r.t);
    }
    t
}
