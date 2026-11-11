//! Allocation of session-unique ids for surfaces, commits, windows, clients, buffers, outputs,
//! input events, probes and scenario runs.

use fakeland_types::{
    BufferId, ClientId, CommitId, InputEventId, OutputId, ProbeId, RunId, SurfaceId, WindowId,
};

/// Counters for every id kind. Ids start at 1 and never repeat within a session.
#[derive(Debug, Default)]
pub struct Ids {
    surface: u64,
    commit: u64,
    window: u64,
    client: u64,
    buffer: u64,
    output: u32,
    input: u64,
    probe: u64,
    run: u64,
}

impl Ids {
    /// Returns a new surface id.
    pub fn surface(&mut self) -> SurfaceId {
        self.surface += 1;
        SurfaceId(self.surface)
    }

    /// Returns a new commit id.
    pub fn commit(&mut self) -> CommitId {
        self.commit += 1;
        CommitId(self.commit)
    }

    /// Returns a new window id.
    pub fn window(&mut self) -> WindowId {
        self.window += 1;
        WindowId(self.window)
    }

    /// Returns a new client id.
    pub fn client(&mut self) -> ClientId {
        self.client += 1;
        ClientId(self.client)
    }

    /// Returns a new buffer id.
    pub fn buffer(&mut self) -> BufferId {
        self.buffer += 1;
        BufferId(self.buffer)
    }

    /// Returns a new output id.
    pub fn output(&mut self) -> OutputId {
        self.output += 1;
        OutputId(self.output)
    }

    /// Returns a new input event id.
    pub fn input(&mut self) -> InputEventId {
        self.input += 1;
        InputEventId(self.input)
    }

    /// Returns a new probe id.
    pub fn probe(&mut self) -> ProbeId {
        self.probe += 1;
        ProbeId(self.probe)
    }

    /// Returns a new scenario run id.
    pub fn run(&mut self) -> RunId {
        self.run += 1;
        RunId(self.run)
    }
}
