//! Test fixtures: a builder for hand-written event sequences on a 60 Hz output.

use fakeland_types::{
    BufferId, ClientId, CommitId, InputEventId, Nanos, OutputId, Physical, ProcessId, Rect,
    SurfaceId, Timestamp, WindowId,
};

use crate::event::{
    AppMarkKind, BufferInfo, BufferKind, Environment, Event, InputKind, InputSource, OutputInfo,
    SyncKind,
};
use crate::trace::{Trace, TraceHeader};

/// The refresh period of the fixture output.
pub(crate) const PERIOD: Nanos = Nanos(16_666_667);

/// The fixture output.
pub(crate) const OUT: OutputId = OutputId(0);

/// Returns the time of vblank `n` on the fixture output, starting at one second.
pub(crate) fn vblank_time(n: u64) -> Timestamp {
    Timestamp(1_000_000_000) + Nanos(PERIOD.0 * n as i64)
}

/// The fixture output configuration.
pub(crate) fn output() -> OutputInfo {
    OutputInfo {
        output: OUT,
        name: "DP-1".into(),
        width: 1920,
        height: 1080,
        refresh_mhz: 60_000,
        scale: 1.0,
        transform: "normal".into(),
        timing: "fixed 60 Hz".into(),
    }
}

/// A builder of event sequences.
pub(crate) struct Fixture {
    trace: Trace,
}

impl Fixture {
    /// Builds a fixture with the session, the output, one process and one client.
    pub(crate) fn new() -> Self {
        let header = TraceHeader::new(
            Environment {
                gpu: "test gpu".into(),
                ..Environment::default()
            },
            vec![output()],
            Timestamp(1_000_000_000),
        );
        let mut f = Self {
            trace: Trace::new(header),
        };
        let t = Timestamp(1_000_000_000);
        f.push(
            t,
            Event::SessionStarted {
                format_version: crate::event::TRACE_FORMAT_VERSION,
                environment: Environment::default(),
            },
        );
        f.push(t, Event::OutputConfigured(output()));
        f.push(
            t,
            Event::ProcessSpawned {
                pid: ProcessId(4242),
                command: "app --flag".into(),
            },
        );
        f.push(
            t,
            Event::ClientConnected {
                client: ClientId(1),
                pid: Some(ProcessId(4242)),
            },
        );
        f
    }

    /// Appends an event.
    pub(crate) fn push(&mut self, t: Timestamp, event: Event) -> &mut Self {
        self.trace.push(t, event);
        self
    }

    /// Creates a toplevel surface and maps its window.
    pub(crate) fn toplevel(&mut self, t: Timestamp, s: u64, w: u64) -> &mut Self {
        self.push(
            t,
            Event::SurfaceCreated {
                surface: SurfaceId(s),
                client: ClientId(1),
            },
        );
        self.push(
            t,
            Event::SurfaceRole {
                surface: SurfaceId(s),
                role: "xdg_toplevel".into(),
                parent: None,
            },
        );
        self.push(
            t,
            Event::WindowMapped {
                window: WindowId(w),
                surface: SurfaceId(s),
                app_id: "org.test.app".into(),
                title: "Test".into(),
                output: Some(OUT),
            },
        )
    }

    /// Creates a subsurface of a parent.
    pub(crate) fn subsurface(&mut self, t: Timestamp, s: u64, parent: u64) -> &mut Self {
        self.push(
            t,
            Event::SurfaceCreated {
                surface: SurfaceId(s),
                client: ClientId(1),
            },
        );
        self.push(
            t,
            Event::SurfaceRole {
                surface: SurfaceId(s),
                role: "subsurface".into(),
                parent: Some(SurfaceId(parent)),
            },
        )
    }

    /// Appends a vblank of the fixture output.
    pub(crate) fn vblank(&mut self, n: u64, late: Nanos) -> &mut Self {
        let ideal = vblank_time(n);
        self.push(
            ideal + late,
            Event::Vblank {
                output: OUT,
                seq: n,
                ideal,
                woke: ideal + late,
                dropped: false,
            },
        )
    }

    /// Appends a frame callback.
    pub(crate) fn callback(&mut self, t: Timestamp, s: u64) -> &mut Self {
        self.push(
            t,
            Event::FrameCallbackSent {
                surface: SurfaceId(s),
                time: t,
            },
        )
    }

    /// Appends a commit with a full-damage 100x100 buffer.
    pub(crate) fn commit(&mut self, t: Timestamp, s: u64, c: u64, sync: SyncKind) -> &mut Self {
        self.push(t, commit_event(s, c, sync, Some(100)))
    }

    /// Appends a commit without a new buffer.
    pub(crate) fn empty_commit(&mut self, t: Timestamp, s: u64, c: u64) -> &mut Self {
        self.push(t, commit_event(s, c, SyncKind::None, None))
    }

    /// Appends a ready record.
    pub(crate) fn ready(&mut self, t: Timestamp, s: u64, c: u64) -> &mut Self {
        self.push(
            t,
            Event::Ready {
                surface: SurfaceId(s),
                commit: CommitId(c),
            },
        )
    }

    /// Appends a latch for vblank `seq`.
    pub(crate) fn latch(&mut self, t: Timestamp, s: u64, c: u64, seq: u64) -> &mut Self {
        self.push(
            t,
            Event::Latched {
                surface: SurfaceId(s),
                commit: CommitId(c),
                output: OUT,
                seq,
            },
        )
    }

    /// Appends a presentation at vblank `seq`.
    pub(crate) fn present(&mut self, s: u64, c: u64, seq: u64) -> &mut Self {
        let time = vblank_time(seq);
        self.push(
            time,
            Event::Presented {
                surface: SurfaceId(s),
                commit: CommitId(c),
                output: OUT,
                seq,
                time,
                refresh: Some(PERIOD),
                flags: 0x7,
            },
        )
    }

    /// Appends a discard.
    pub(crate) fn discard(&mut self, t: Timestamp, s: u64, c: u64) -> &mut Self {
        self.push(
            t,
            Event::Discarded {
                surface: SurfaceId(s),
                commit: CommitId(c),
            },
        )
    }

    /// Appends a buffer release.
    pub(crate) fn release(&mut self, t: Timestamp, s: u64, c: u64) -> &mut Self {
        self.push(
            t,
            Event::Released {
                surface: SurfaceId(s),
                commit: CommitId(c),
            },
        )
    }

    /// Appends an input event sent to a surface.
    pub(crate) fn input(&mut self, t: Timestamp, id: u64, s: u64, kind: InputKind) -> &mut Self {
        self.push(
            t,
            Event::InputSent {
                id: InputEventId(id),
                kind,
                surface: Some(SurfaceId(s)),
                x: Some(10.0),
                y: Some(20.0),
                detail: String::new(),
                source: InputSource::Api,
            },
        )
    }

    /// Appends an app mark.
    pub(crate) fn mark(
        &mut self,
        t: Timestamp,
        s: Option<u64>,
        c: Option<u64>,
        mark: AppMarkKind,
    ) -> &mut Self {
        self.push(
            t,
            Event::AppMark {
                surface: s.map(SurfaceId),
                commit: c.map(CommitId),
                mark,
            },
        )
    }

    /// Returns the trace.
    pub(crate) fn build(&self) -> Trace {
        self.trace.clone()
    }
}

/// Builds a commit event. A size gives a square dmabuf with full damage.
pub(crate) fn commit_event(s: u64, c: u64, sync: SyncKind, size: Option<i32>) -> Event {
    Event::Commit {
        surface: SurfaceId(s),
        commit: CommitId(c),
        buffer: size.map(|n| BufferInfo {
            id: BufferId(c % 3),
            kind: BufferKind::Dmabuf,
            format: u32::from_le_bytes(*b"XR24"),
            modifier: Some(0),
            width: n,
            height: n,
            scale: 1.0,
            viewport: None,
        }),
        damage: size
            .map(|_| vec![Rect::<Physical>::new(0, 0, i32::MAX, i32::MAX)])
            .unwrap_or_default(),
        frame_callback: true,
        presentation_feedback: true,
        sync,
        fifo_barrier: false,
        fifo_wait: false,
        target: None,
        configure_serial: Some(1),
        detached: false,
    }
}

/// Builds a steady 60 Hz flow on surface 1: for each frame `n` in `0..frames`, a callback at
/// vblank `n`, a commit 5 ms later with explicit sync, ready 2 ms after the commit, latch 1.5 ms
/// before vblank `n + 1`, and presentation at vblank `n + 1`. The previous buffer releases at
/// each presentation.
pub(crate) fn steady(frames: u64) -> Fixture {
    let mut f = Fixture::new();
    f.toplevel(vblank_time(0), 1, 1);
    for n in 0..frames {
        let v = vblank_time(n);
        let c = n + 1;
        if n > 0 {
            f.present(1, c - 1, n);
            if n > 1 {
                f.release(vblank_time(n), 1, c - 2);
            }
        }
        f.vblank(n, Nanos(20_000));
        f.callback(v + Nanos(30_000), 1);
        f.commit(v + Nanos::from_millis(5), 1, c, SyncKind::Explicit);
        f.ready(v + Nanos::from_millis(7), 1, c);
        f.latch(vblank_time(n + 1) - Nanos::from_micros(1500), 1, c, n + 1);
    }
    f.present(1, frames, frames);
    f.vblank(frames, Nanos(20_000));
    f
}

/// Returns one event of every variant, `Unknown` excluded.
pub(crate) fn every_event() -> Vec<Event> {
    use fakeland_types::{Logical, ProbeId, RunId, Size};

    let s = SurfaceId(1);
    let c = CommitId(2);
    vec![
        Event::SessionStarted {
            format_version: crate::event::TRACE_FORMAT_VERSION,
            environment: Environment {
                gpu: "RTX".into(),
                driver: "nvidia 610".into(),
                kernel: "7.1".into(),
                cpu: "x86".into(),
                governor: "performance".into(),
                renderer: "gles".into(),
                hostname: "host".into(),
                fakeland_version: "0.1.0".into(),
            },
        },
        Event::OutputConfigured(output()),
        Event::OutputChanged(OutputInfo {
            scale: 1.5,
            ..output()
        }),
        Event::OutputPower {
            output: OUT,
            on: false,
        },
        Event::Vblank {
            output: OUT,
            seq: 3,
            ideal: Timestamp(10),
            woke: Timestamp(12),
            dropped: true,
        },
        Event::ClientConnected {
            client: ClientId(1),
            pid: Some(ProcessId(9)),
        },
        Event::ProcessSpawned {
            pid: ProcessId(9),
            command: "vkcube --wsi wayland".into(),
        },
        Event::SurfaceCreated {
            surface: s,
            client: ClientId(1),
        },
        Event::SurfaceRole {
            surface: s,
            role: "xdg_toplevel".into(),
            parent: None,
        },
        Event::WindowMapped {
            window: WindowId(1),
            surface: s,
            app_id: "vkcube".into(),
            title: "vkcube".into(),
            output: Some(OUT),
        },
        Event::WindowTitle {
            window: WindowId(1),
            title: "new \"title\"".into(),
        },
        Event::WindowAppId {
            window: WindowId(1),
            app_id: "cube".into(),
        },
        Event::Focus { surface: Some(s) },
        Event::Configure {
            surface: s,
            serial: 5,
            width: 800,
            height: 600,
            states: vec!["maximized".into(), "activated".into()],
        },
        Event::AckConfigure {
            surface: s,
            serial: 5,
        },
        Event::FrameCallbackSent {
            surface: s,
            time: Timestamp(20),
        },
        Event::InputSent {
            id: InputEventId(1),
            kind: InputKind::PointerAxis,
            surface: Some(s),
            x: Some(0.1 + 0.2),
            y: Some(-3.75),
            detail: "vertical 120".into(),
            source: InputSource::Scenario { run: RunId(4) },
        },
        Event::AppMark {
            surface: Some(s),
            commit: Some(c),
            mark: AppMarkKind::Span {
                name: "render".into(),
                begin: Timestamp(21),
                end: Timestamp(25),
            },
        },
        Event::AppMark {
            surface: Some(s),
            commit: Some(c),
            mark: AppMarkKind::InputHandled {
                input: InputEventId(1),
            },
        },
        Event::AppMark {
            surface: Some(s),
            commit: Some(c),
            mark: AppMarkKind::Frame { frame_id: 1 },
        },
        Event::AppMark {
            surface: Some(s),
            commit: None,
            mark: AppMarkKind::Mark {
                name: "idle".into(),
            },
        },
        Event::Commit {
            surface: s,
            commit: c,
            buffer: Some(BufferInfo {
                id: BufferId(3),
                kind: BufferKind::Dmabuf,
                format: 0x3432_5258,
                modifier: Some(u64::MAX - 1),
                width: 800,
                height: 600,
                scale: 1.25,
                viewport: Some(Size::<Logical>::new(640, 480)),
            }),
            damage: vec![Rect::new(1, 2, 3, 4)],
            frame_callback: true,
            presentation_feedback: true,
            sync: SyncKind::Explicit,
            fifo_barrier: true,
            fifo_wait: true,
            target: Some(Timestamp(40)),
            configure_serial: Some(5),
            detached: false,
        },
        Event::Ready {
            surface: s,
            commit: c,
        },
        Event::Latched {
            surface: s,
            commit: c,
            output: OUT,
            seq: 4,
        },
        Event::Presented {
            surface: s,
            commit: c,
            output: OUT,
            seq: 4,
            time: Timestamp(50),
            refresh: Some(PERIOD),
            flags: 0xf,
        },
        Event::Discarded {
            surface: s,
            commit: CommitId(1),
        },
        Event::Released {
            surface: s,
            commit: c,
        },
        Event::UserMark {
            label: "start, scroll".into(),
        },
        Event::ScenarioStarted {
            run: RunId(4),
            name: "scroll".into(),
        },
        Event::ScenarioStep {
            run: RunId(4),
            index: 0,
            label: "glide".into(),
        },
        Event::ScenarioFinished {
            run: RunId(4),
            ok: false,
            error: Some("timeout".into()),
        },
        Event::ProbeStarted {
            probe: ProbeId(1),
            kind: "pointer_move".into(),
        },
        Event::ProbeInput {
            probe: ProbeId(1),
            input: InputEventId(1),
        },
        Event::ProbeDetected {
            probe: ProbeId(1),
            commit: c,
            detector: "any_damage".into(),
        },
        Event::ProbeTimedOut { probe: ProbeId(2) },
        Event::Capture {
            what: "output DP-1".into(),
            cost: Nanos::from_micros(800),
        },
        Event::CompositorOverhead {
            what: "readback".into(),
            duration: Nanos::from_micros(300),
        },
        Event::ProcessSample {
            pid: ProcessId(9),
            cpu_percent: 12.5,
            rss_bytes: 1 << 30,
            threads: 12,
        },
        Event::GpuSample {
            util_percent: 55.0,
            mem_used_bytes: 1 << 31,
            mem_total_bytes: 12 << 30,
        },
        Event::Clipboard {
            mime: vec!["text/plain".into()],
            primary: false,
        },
        Event::DeadlineWake {
            output: OUT,
            deadline: Timestamp(990_000_000),
            woke: Timestamp(990_050_000),
        },
        Event::ProcessDetail {
            pid: ProcessId(9),
            cpu_ns_total: 2_000_000_000,
            ctx_switches_total: 5_000,
            pss_bytes: Some(512 << 20),
            private_bytes: Some(256 << 20),
            swap_bytes: Some(0),
            hwm_bytes: Some(1 << 30),
        },
        Event::GpuProcessSample {
            pid: ProcessId(9),
            mem_bytes: Some(300 << 20),
            busy_ns_total: Some(40_000_000),
            util_percent: None,
            source: "fdinfo:amdgpu".into(),
        },
        Event::EnergySample {
            domain: "cpu-package-0".into(),
            microjoules_total: 7_000_000,
        },
        Event::WindowUnmapped {
            window: WindowId(1),
        },
        Event::SurfaceDestroyed { surface: s },
        Event::ClientDisconnected {
            client: ClientId(1),
        },
        Event::ProcessExited {
            pid: ProcessId(9),
            code: None,
            signal: Some(15),
        },
        Event::OutputRemoved { output: OUT },
    ]
}

/// Returns a trace with every event variant at increasing times.
pub(crate) fn every_event_trace() -> Trace {
    let mut trace = Trace::new(TraceHeader::new(
        Environment::default(),
        vec![output()],
        Timestamp(1_000),
    ));
    for (i, e) in every_event().into_iter().enumerate() {
        trace.push(Timestamp(1_000 + i as u64 * 1_000), e);
    }
    trace
}
