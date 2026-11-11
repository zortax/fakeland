//! Test fixtures: a builder of synthetic traces with one window on one output.

use fakeland_trace::{
    AppMarkKind, BufferInfo, BufferKind, Environment, Event, InputKind, InputSource, OutputInfo,
    Record, SyncKind, Trace, TraceHeader,
};
use fakeland_types::{
    BufferId, ClientId, CommitId, InputEventId, Logical, Nanos, OutputId, Physical, ProcessId,
    Rect, SurfaceId, Timestamp, WindowId,
};

use crate::input::{AnalysisInput, Target};

/// The start time of every fixture.
pub(crate) const T0: Timestamp = Timestamp(1_000_000_000);

/// The fixture output.
pub(crate) const OUT: OutputId = OutputId(0);

/// The fixture window.
pub(crate) const WIN: WindowId = WindowId(1);

/// The fixture toplevel surface.
pub(crate) const SURF: SurfaceId = SurfaceId(1);

/// Returns `ms` milliseconds after the fixture start.
pub(crate) fn at_ms(ms: f64) -> Timestamp {
    T0 + Nanos::from_millis_f64(ms)
}

/// The options of one commit.
#[derive(Clone, Debug)]
pub(crate) struct CommitOpts {
    pub surface: SurfaceId,
    pub buffer: bool,
    pub size: Option<(i32, i32)>,
    pub kind: BufferKind,
    pub buffer_id: Option<u64>,
    pub damage: Vec<Rect<Physical>>,
    pub sync: SyncKind,
    pub target: Option<Timestamp>,
    pub configure_serial: Option<u32>,
    pub frame_callback: bool,
    pub viewport: Option<(i32, i32)>,
    pub buffer_scale: f64,
}

impl Default for CommitOpts {
    fn default() -> Self {
        Self {
            surface: SURF,
            buffer: true,
            size: None,
            kind: BufferKind::Shm,
            buffer_id: None,
            damage: vec![Rect::new(0, 0, 10, 10)],
            sync: SyncKind::None,
            target: None,
            configure_serial: None,
            frame_callback: true,
            viewport: None,
            buffer_scale: 1.0,
        }
    }
}

/// A synthetic trace builder. Records sort by time when the trace is built.
pub(crate) struct Synth {
    records: Vec<Record>,
    pub period: Nanos,
    pub scale: f64,
    pub size: (i32, i32),
    next_commit: u64,
    next_input: u64,
    next_buffer: u64,
    next_surface: u64,
    surface_of: std::collections::HashMap<CommitId, SurfaceId>,
}

impl Synth {
    /// Builds a fixture with one 1280x720 window on a 60 Hz output.
    pub(crate) fn new() -> Self {
        Self::with_output(60.0, 1.0)
    }

    /// Builds a fixture with a refresh rate and a scale.
    pub(crate) fn with_output(hz: f64, scale: f64) -> Self {
        let mut s = Self {
            records: Vec::new(),
            period: Nanos::from_secs_f64(1.0 / hz),
            scale,
            size: (1280, 720),
            next_commit: 1,
            next_input: 1,
            next_buffer: 1,
            next_surface: 2,
            surface_of: Default::default(),
        };
        s.push(
            T0,
            Event::SessionStarted {
                format_version: fakeland_trace::TRACE_FORMAT_VERSION,
                environment: Environment::default(),
            },
        );
        s.push(T0, Event::OutputConfigured(s.output_info(hz)));
        s.push(
            T0,
            Event::ProcessSpawned {
                pid: ProcessId(100),
                command: "app".into(),
            },
        );
        s.push(
            T0,
            Event::ClientConnected {
                client: ClientId(1),
                pid: Some(ProcessId(100)),
            },
        );
        s.push(
            T0,
            Event::SurfaceCreated {
                surface: SURF,
                client: ClientId(1),
            },
        );
        s.push(
            T0,
            Event::SurfaceRole {
                surface: SURF,
                role: "xdg_toplevel".into(),
                parent: None,
            },
        );
        s.push(
            T0,
            Event::WindowMapped {
                window: WIN,
                surface: SURF,
                app_id: "app".into(),
                title: "App".into(),
                output: Some(OUT),
            },
        );
        s
    }

    fn output_info(&self, hz: f64) -> OutputInfo {
        OutputInfo {
            output: OUT,
            name: "DP-1".into(),
            width: 1920,
            height: 1080,
            refresh_mhz: (hz * 1000.0).round() as u32,
            scale: self.scale,
            transform: "normal".into(),
            timing: "fixed".into(),
        }
    }

    /// Adds a record.
    pub(crate) fn push(&mut self, t: Timestamp, event: Event) {
        self.records.push(Record::new(t, event));
    }

    /// Returns the time of vblank `seq`; vblank 0 is the fixture start.
    pub(crate) fn vblank(&self, seq: u64) -> Timestamp {
        T0 + Nanos(self.period.as_nanos() * seq as i64)
    }

    /// Adds vblank records for a range of sequences with a wake lateness.
    pub(crate) fn vblanks(&mut self, seqs: std::ops::RangeInclusive<u64>, late: Nanos) {
        for seq in seqs {
            let ideal = self.vblank(seq);
            self.push(
                ideal + late,
                Event::Vblank {
                    output: OUT,
                    seq,
                    ideal,
                    woke: ideal + late,
                    dropped: false,
                },
            );
        }
    }

    /// Adds a surface with a role and a parent.
    pub(crate) fn surface(
        &mut self,
        t: Timestamp,
        role: &str,
        parent: Option<SurfaceId>,
    ) -> SurfaceId {
        let s = SurfaceId(self.next_surface);
        self.next_surface += 1;
        self.push(
            t,
            Event::SurfaceCreated {
                surface: s,
                client: ClientId(1),
            },
        );
        self.push(
            t,
            Event::SurfaceRole {
                surface: s,
                role: role.into(),
                parent,
            },
        );
        s
    }

    /// Adds a commit.
    pub(crate) fn commit(&mut self, t: Timestamp, o: CommitOpts) -> CommitId {
        let c = CommitId(self.next_commit);
        self.next_commit += 1;
        let (w, h) = o.size.unwrap_or((
            (self.size.0 as f64 * self.scale).round() as i32,
            (self.size.1 as f64 * self.scale).round() as i32,
        ));
        let id = o.buffer_id.unwrap_or_else(|| {
            let id = self.next_buffer % 3;
            self.next_buffer += 1;
            id
        });
        let buffer = o.buffer.then(|| BufferInfo {
            id: BufferId(id),
            kind: o.kind,
            format: u32::from_le_bytes(*b"XR24"),
            modifier: None,
            width: w,
            height: h,
            scale: o.buffer_scale,
            viewport: o
                .viewport
                .map(|(w, h)| fakeland_types::Size::<Logical>::new(w, h)),
        });
        self.surface_of.insert(c, o.surface);
        self.push(
            t,
            Event::Commit {
                surface: o.surface,
                commit: c,
                buffer,
                damage: if o.buffer { o.damage } else { Vec::new() },
                frame_callback: o.frame_callback,
                presentation_feedback: true,
                sync: o.sync,
                fifo_barrier: false,
                fifo_wait: false,
                target: o.target,
                configure_serial: o.configure_serial,
                detached: false,
            },
        );
        c
    }

    fn surface_of(&self, c: CommitId) -> SurfaceId {
        self.surface_of.get(&c).copied().unwrap_or(SURF)
    }

    /// Records a fence ready time.
    pub(crate) fn ready(&mut self, c: CommitId, t: Timestamp) {
        let surface = self.surface_of(c);
        self.push(t, Event::Ready { surface, commit: c });
    }

    /// Latches a commit 1 ms before vblank `seq` and presents it at that vblank.
    pub(crate) fn present(&mut self, c: CommitId, seq: u64) {
        let surface = self.surface_of(c);
        let time = self.vblank(seq);
        self.push(
            time - Nanos::from_millis(1),
            Event::Latched {
                surface,
                commit: c,
                output: OUT,
                seq,
            },
        );
        self.push(
            time,
            Event::Presented {
                surface,
                commit: c,
                output: OUT,
                seq,
                time,
                refresh: Some(self.period),
                flags: 7,
            },
        );
    }

    /// Discards a commit.
    pub(crate) fn discard(&mut self, c: CommitId, t: Timestamp) {
        let surface = self.surface_of(c);
        self.push(t, Event::Discarded { surface, commit: c });
    }

    /// Releases the buffer of a commit.
    pub(crate) fn release(&mut self, c: CommitId, t: Timestamp) {
        let surface = self.surface_of(c);
        self.push(t, Event::Released { surface, commit: c });
    }

    /// Commits a default frame at `t` and presents it at vblank `seq`.
    pub(crate) fn frame(&mut self, t: Timestamp, seq: u64) -> CommitId {
        let c = self.commit(t, CommitOpts::default());
        self.present(c, seq);
        c
    }

    /// Records an injected input event.
    pub(crate) fn input(
        &mut self,
        t: Timestamp,
        kind: InputKind,
        pos: Option<(f64, f64)>,
        detail: &str,
    ) -> InputEventId {
        let id = InputEventId(self.next_input);
        self.next_input += 1;
        self.push(
            t,
            Event::InputSent {
                id,
                kind,
                surface: Some(SURF),
                x: pos.map(|p| p.0),
                y: pos.map(|p| p.1),
                detail: detail.into(),
                source: InputSource::Api,
            },
        );
        id
    }

    /// Records an app mark on a commit.
    pub(crate) fn mark(&mut self, c: CommitId, t: Timestamp, mark: AppMarkKind) {
        let surface = self.surface_of(c);
        self.push(
            t,
            Event::AppMark {
                surface: Some(surface),
                commit: Some(c),
                mark,
            },
        );
    }

    /// Records that a commit handled an input.
    pub(crate) fn handled(&mut self, c: CommitId, t: Timestamp, input: InputEventId) {
        self.mark(c, t, AppMarkKind::InputHandled { input });
    }

    /// Records an app frame id on a commit.
    pub(crate) fn frame_id(&mut self, c: CommitId, t: Timestamp, frame_id: u64) {
        self.mark(c, t, AppMarkKind::Frame { frame_id });
    }

    /// Records a configure of the toplevel.
    pub(crate) fn configure(&mut self, t: Timestamp, serial: u32, w: i32, h: i32, states: &[&str]) {
        self.push(
            t,
            Event::Configure {
                surface: SURF,
                serial,
                width: w,
                height: h,
                states: states.iter().map(|s| s.to_string()).collect(),
            },
        );
    }

    /// Records an ack of a configure.
    pub(crate) fn ack(&mut self, t: Timestamp, serial: u32) {
        self.push(
            t,
            Event::AckConfigure {
                surface: SURF,
                serial,
            },
        );
    }

    /// Builds the trace.
    pub(crate) fn trace(&self) -> Trace {
        let mut records = self.records.clone();
        records.sort_by_key(|r| r.t);
        let mut trace = Trace::new(TraceHeader::new(
            Environment::default(),
            vec![self.output_info(1e9 / self.period.as_nanos() as f64)],
            T0,
        ));
        trace.records = records;
        trace
    }

    /// Builds an analysis input with the fixture window as the target.
    pub(crate) fn analysis(&self) -> AnalysisInput {
        AnalysisInput::new(self.trace()).with_target(Some(Target {
            window: WIN,
            surface: SURF,
            output: OUT,
            geometry: Rect::new(0, 0, self.size.0, self.size.1),
            app_id: "app".into(),
            pid: Some(ProcessId(100)),
            app: Some(ProcessId(100)),
        }))
    }
}

/// Builds a texture with structure along both axes: smooth blocks of seeded noise.
pub(crate) fn texture(w: u32, h: u32, seed: u64) -> fakeland_vision::Image {
    let mut data = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let a = fakeland_types::SplitMix64::hash(seed, u64::from(x / 5), u64::from(y / 4));
            let b =
                fakeland_types::SplitMix64::hash(seed ^ 1, u64::from(x / 13), u64::from(y / 11));
            let v = ((a % 160) + (b % 96)) as u8;
            data.extend_from_slice(&[v, v / 2 + 40, 255 - v, 255]);
        }
    }
    fakeland_vision::Image::from_rgba(w, h, data).expect("texture")
}

/// Wraps an image as a captured presented frame of the fixture window.
pub(crate) fn captured(
    image: fakeland_vision::Image,
    commit: CommitId,
    presented: Timestamp,
    seq: u64,
) -> fakeland::CapturedFrame {
    let (w, h) = (image.width() as i32, image.height() as i32);
    fakeland::CapturedFrame {
        commit,
        surface: SURF,
        window: WIN,
        time: presented,
        presented: Some(presented),
        seq: Some(seq),
        image,
        region: Rect::new(0, 0, w, h),
        scale: 1.0,
    }
}

/// Wraps captured frames as a capture of presented frames.
pub(crate) fn capture_of(frames: Vec<fakeland::CapturedFrame>) -> fakeland::FrameCapture {
    fakeland::FrameCapture {
        spec: fakeland::CaptureSpec::every_presented(fakeland::WindowRef::id(WIN)),
        frames,
        dropped: 0,
        cost: Nanos::ZERO,
    }
}

/// Wraps captured frames as a capture of every commit.
pub(crate) fn commit_capture_of(frames: Vec<fakeland::CapturedFrame>) -> fakeland::FrameCapture {
    fakeland::FrameCapture {
        spec: fakeland::CaptureSpec::every_commit(fakeland::WindowRef::id(WIN)),
        frames,
        dropped: 0,
        cost: Nanos::ZERO,
    }
}
