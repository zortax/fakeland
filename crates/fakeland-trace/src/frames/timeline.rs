//! The frame timeline and its queries.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::ops::RangeBounds;

use fakeland_types::{
    ClientId, CommitId, InputEventId, OutputId, ProcessId, SurfaceId, Timestamp, WindowId,
};
use serde::Serialize;

use crate::event::OutputInfo;
use crate::frames::builder::Builder;
use crate::frames::{
    ClientInfo, FrameRecord, InputRecord, PresentedFrame, ProcessInfo, SurfaceInfo, VblankInfo,
    WindowInfo, attribution, derive,
};
use crate::trace::{SurfaceTree, Trace};

/// Frames per surface and the entities of a trace.
///
/// Each surface, subsurfaces included, has its own frame list in commit order. Subsurfaces and
/// popups link to their parents through the surface tree.
#[derive(Clone, Debug, Default, Serialize)]
pub struct FrameTimeline {
    pub(crate) surfaces: BTreeMap<SurfaceId, SurfaceInfo>,
    pub(crate) frames: BTreeMap<SurfaceId, Vec<FrameRecord>>,
    pub(crate) windows: BTreeMap<WindowId, WindowInfo>,
    pub(crate) clients: BTreeMap<ClientId, ClientInfo>,
    pub(crate) processes: BTreeMap<ProcessId, ProcessInfo>,
    pub(crate) outputs: BTreeMap<OutputId, OutputInfo>,
    pub(crate) vblanks: BTreeMap<OutputId, Vec<VblankInfo>>,
    pub(crate) inputs: Vec<InputRecord>,
    pub(crate) orphans: usize,
    pub(crate) global_marks: usize,
    #[serde(skip)]
    pub(crate) index: HashMap<CommitId, (SurfaceId, usize)>,
    #[serde(skip)]
    pub(crate) input_index: HashMap<InputEventId, usize>,
    #[serde(skip)]
    pub(crate) tree: SurfaceTree,
}

impl FrameTimeline {
    /// Builds the timeline of a trace.
    pub fn build(trace: &Trace) -> Self {
        let mut tl = Builder::run(trace);
        derive::fill(&mut tl);
        attribution::attribute(&mut tl);
        tl
    }

    /// Returns every known surface in id order.
    pub fn surfaces(&self) -> impl Iterator<Item = &SurfaceInfo> + '_ {
        self.surfaces.values()
    }

    /// Returns a surface.
    pub fn surface(&self, surface: SurfaceId) -> Option<&SurfaceInfo> {
        self.surfaces.get(&surface)
    }

    /// Returns the surfaces with at least one commit, in id order.
    pub fn committed_surfaces(&self) -> impl Iterator<Item = SurfaceId> + '_ {
        self.frames.keys().copied()
    }

    /// Returns the frames of a surface in commit order.
    pub fn frames(&self, surface: SurfaceId) -> &[FrameRecord] {
        self.frames.get(&surface).map_or(&[], Vec::as_slice)
    }

    /// Returns the frames of a surface whose commit time lies in a range.
    pub fn frames_in<'a>(
        &'a self,
        surface: SurfaceId,
        range: impl RangeBounds<Timestamp> + 'a,
    ) -> impl Iterator<Item = &'a FrameRecord> + 'a {
        self.frames(surface)
            .iter()
            .filter(move |f| range.contains(&f.committed))
    }

    /// Returns every frame, by surface and then commit order.
    pub fn all_frames(&self) -> impl Iterator<Item = &FrameRecord> + '_ {
        self.frames.values().flatten()
    }

    /// Returns the frame of a commit.
    pub fn frame(&self, commit: CommitId) -> Option<&FrameRecord> {
        let (s, i) = self.index.get(&commit)?;
        self.frames.get(s)?.get(*i)
    }

    pub(crate) fn frame_mut(&mut self, commit: CommitId) -> Option<&mut FrameRecord> {
        let (s, i) = self.index.get(&commit)?;
        self.frames.get_mut(s)?.get_mut(*i)
    }

    /// Returns the presented frames of a surface in commit order: the commits whose new content
    /// a refresh shows. Presented commits without content are left out.
    pub fn presented(&self, surface: SurfaceId) -> impl Iterator<Item = &FrameRecord> + '_ {
        self.frames(surface)
            .iter()
            .filter(|f| f.is_presented_frame())
    }

    /// Returns the presented frames of a surface in presentation order, on one output or on all
    /// outputs. Intervals and missed vblanks count within the returned sequence.
    pub fn presented_frames(
        &self,
        surface: SurfaceId,
        output: Option<OutputId>,
    ) -> Vec<PresentedFrame> {
        let mut out: Vec<PresentedFrame> = self
            .presented(surface)
            .filter_map(|f| {
                let p = f.presented?;
                (output.is_none() || output == Some(p.output)).then_some(PresentedFrame {
                    commit: f.commit,
                    surface,
                    output: p.output,
                    seq: p.seq,
                    time: p.time,
                    committed: f.committed,
                    interval: None,
                    vblanks_missed: None,
                })
            })
            .collect();
        out.sort_by_key(|p| (p.time, p.seq, p.commit));
        for i in 1..out.len() {
            let prev = out[i - 1];
            out[i].interval = Some(out[i].time - prev.time);
            out[i].vblanks_missed =
                (prev.output == out[i].output).then(|| out[i].seq.saturating_sub(prev.seq + 1));
        }
        out
    }

    /// Returns the presented frames of every surface on an output in presentation order.
    /// Intervals and missed vblanks count per surface.
    pub fn presented_on(&self, output: OutputId) -> Vec<PresentedFrame> {
        let mut out: Vec<PresentedFrame> = self
            .committed_surfaces()
            .flat_map(|s| self.presented_frames(s, Some(output)))
            .collect();
        out.sort_by_key(|p| (p.time, p.surface, p.commit));
        out
    }

    /// Returns the first presented frame of a surface at or after a time.
    pub fn next_presented(&self, surface: SurfaceId, at: Timestamp) -> Option<&FrameRecord> {
        self.presented(surface)
            .filter(|f| f.presented.is_some_and(|p| p.time >= at))
            .min_by_key(|f| f.presented.map(|p| p.time))
    }

    /// Returns every window in id order.
    pub fn windows(&self) -> impl Iterator<Item = &WindowInfo> + '_ {
        self.windows.values()
    }

    /// Returns a window.
    pub fn window(&self, window: WindowId) -> Option<&WindowInfo> {
        self.windows.get(&window)
    }

    /// Returns the toplevel surface of a window.
    pub fn window_surface(&self, window: WindowId) -> Option<SurfaceId> {
        self.windows.get(&window).map(|w| w.surface)
    }

    /// Returns the toplevel surface, subsurfaces and popups of a window.
    pub fn window_surfaces(&self, window: WindowId) -> BTreeSet<SurfaceId> {
        self.window_surface(window)
            .map(|s| self.tree.subtree(s))
            .unwrap_or_default()
    }

    /// Returns the window whose surface tree holds a surface.
    pub fn window_of(&self, surface: SurfaceId) -> Option<WindowId> {
        let root = self.tree.root(surface);
        self.windows
            .values()
            .filter(|w| w.surface == root)
            .map(|w| w.window)
            .max()
    }

    /// Returns the surface tree.
    pub const fn tree(&self) -> &SurfaceTree {
        &self.tree
    }

    /// Returns the direct children of a surface.
    pub fn children(&self, surface: SurfaceId) -> Vec<SurfaceId> {
        self.tree.children(surface)
    }

    /// Returns every client in id order.
    pub fn clients(&self) -> impl Iterator<Item = &ClientInfo> + '_ {
        self.clients.values()
    }

    /// Returns a client.
    pub fn client(&self, client: ClientId) -> Option<&ClientInfo> {
        self.clients.get(&client)
    }

    /// Returns every launched process in pid order.
    pub fn processes(&self) -> impl Iterator<Item = &ProcessInfo> + '_ {
        self.processes.values()
    }

    /// Returns a launched process.
    pub fn process(&self, pid: ProcessId) -> Option<&ProcessInfo> {
        self.processes.get(&pid)
    }

    /// Returns every output in id order, with its latest configuration.
    pub fn outputs(&self) -> impl Iterator<Item = &OutputInfo> + '_ {
        self.outputs.values()
    }

    /// Returns an output.
    pub fn output(&self, output: OutputId) -> Option<&OutputInfo> {
        self.outputs.get(&output)
    }

    /// Returns the vblanks of an output in order.
    pub fn vblanks(&self, output: OutputId) -> &[VblankInfo] {
        self.vblanks.get(&output).map_or(&[], Vec::as_slice)
    }

    /// Returns the outputs that have vblanks.
    pub fn vblank_outputs(&self) -> impl Iterator<Item = OutputId> + '_ {
        self.vblanks.keys().copied()
    }

    /// Returns every input event in send order.
    pub fn inputs(&self) -> &[InputRecord] {
        &self.inputs
    }

    /// Returns an input event.
    pub fn input(&self, id: InputEventId) -> Option<&InputRecord> {
        self.input_index.get(&id).map(|i| &self.inputs[*i])
    }

    /// Returns the number of stage records that name a commit the trace does not hold.
    pub const fn orphans(&self) -> usize {
        self.orphans
    }

    /// Returns the number of app marks that name neither a surface nor a commit.
    pub const fn global_marks(&self) -> usize {
        self.global_marks
    }
}
