//! Detection for probes. The watch reads the trace records after the input: commits of target
//! surfaces, fence readiness, app marks, latches and presentations. Pixel detectors read back the
//! region of each candidate buffer once its fence is ready and compare it with the baseline of
//! the same surface. A content commit of the target between the baselines and the input
//! replaces the baseline of its surface.

use std::collections::{HashMap, HashSet};

use fakeland_trace::{AppMarkKind, BufferInfo, Event, Record, SyncKind};
use fakeland_types::{CommitId, Logical, Nanos, OutputId, Rect, SurfaceId, Timestamp};
use fakeland_vision::Image;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::utils::{Logical as SLogical, Point};

use super::geometry::BufferPlacement;
use super::hit::{detect, take_presentation};
use super::queue::{Candidate, ReadbackQueue, drain_through, enqueue};
use super::readback::committed_placement;
use super::run::{CommitLog, CommitTimes, Shared, Target};
use super::watch::in_target;
use crate::server::State;
use crate::waits::{Condition, mark_name};

/// What a probe detects.
pub(super) enum Goal {
    /// Damage that touches the region.
    Damage,
    /// More than this fraction of the region pixels changed.
    Changed(f64),
    /// The region matches a condition.
    Match(Condition),
    /// An app mark: `None` waits for `input_handled` of the injected input.
    Mark(Option<String>),
}

/// Whether the repeat goes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Step {
    /// Keep watching.
    Continue,
    /// The response is presented.
    Finished,
}

/// The detection state of one repeat.
pub(super) struct Watch {
    pub target: Target,
    pub roi: Option<Rect<Logical>>,
    pub goal: Goal,
    pub started: bool,
    pub stash: HashMap<CommitId, WlBuffer>,
    pub baselines: HashMap<SurfaceId, Image>,
    pub log: CommitLog,
    pub infos: HashMap<CommitId, BufferInfo>,
    pub detected: Option<(CommitId, SurfaceId)>,
    pub follow: bool,
    /// Candidates that wait for their readback.
    pub queue: ReadbackQueue,
    /// Commits from before the input whose fences are pending. Their buffers become baselines.
    pub early: HashSet<CommitId>,
}

impl Watch {
    /// Builds a watch.
    pub fn new(target: Target, roi: Option<Rect<Logical>>, goal: Goal) -> Self {
        Self {
            target,
            roi,
            goal,
            started: false,
            stash: HashMap::new(),
            baselines: HashMap::new(),
            log: CommitLog::new(),
            infos: HashMap::new(),
            detected: None,
            follow: false,
            queue: ReadbackQueue::default(),
            early: HashSet::new(),
        }
    }

    /// Returns true when the detector reads candidate buffers.
    pub fn wants_buffers(&self) -> bool {
        matches!(self.goal, Goal::Changed(_) | Goal::Match(_))
    }

    /// Returns true when the detector compares against baselines.
    pub fn needs_baselines(&self) -> bool {
        match &self.goal {
            Goal::Changed(_) => true,
            Goal::Match(c) => c.needs_baseline(),
            _ => false,
        }
    }

    /// Reads the region from the applied buffer of every target surface. Returns the count of
    /// readbacks and their cost.
    pub fn take_baselines(&mut self, state: &mut State) -> (u32, Nanos) {
        let (Some(roi), true) = (self.roi, self.needs_baselines()) else {
            return (0, Nanos::ZERO);
        };
        let windows: Vec<_> = match self.target {
            Target::Window(w) => vec![w],
            Target::Output(o) => state.windows.on_output(o),
            Target::Any => Vec::new(),
        };
        let mut reads = 0;
        let mut cost = Nanos::ZERO;
        for w in windows {
            for s in state.window_tree(w) {
                let Some((buffer, placement)) = state.applied_placement(&s.surface, s.origin)
                else {
                    continue;
                };
                if let Some(read) = state.read_region(&buffer, &placement, roi) {
                    reads += 1;
                    cost += read.cost;
                    self.baselines.insert(s.id, read.image);
                }
            }
        }
        (reads, cost)
    }
}

/// Returns the global origin of a surface in its window tree.
pub(super) fn surface_origin(state: &State, surface: SurfaceId) -> Option<Point<i32, SLogical>> {
    let w = state.window_of_surface_id(surface)?;
    state
        .window_tree(w)
        .into_iter()
        .find(|s| s.id == surface)
        .map(|s| s.origin)
}

/// Handles one trace record for a repeat. Records from before the input only refresh baselines.
pub(super) fn on_record(state: &mut State, run: &Shared, record: &Record) -> Step {
    let before_input = {
        let r = run.borrow();
        let Some(rep) = r.repeat.as_ref().filter(|rep| !rep.finished) else {
            return Step::Continue;
        };
        !rep.watch.started || rep.result.input_sent.is_none_or(|sent| record.t < sent)
    };
    if before_input {
        return before_the_input(state, run, record);
    }
    match &record.event {
        Event::Commit {
            surface,
            commit,
            buffer,
            damage,
            sync,
            ..
        } => {
            if !in_target(state, run, *surface) {
                return Step::Continue;
            }
            let (detected, goal_damage, goal_pixels, roi) = {
                let mut r = run.borrow_mut();
                let Some(rep) = r.repeat.as_mut() else {
                    return Step::Continue;
                };
                let w = &mut rep.watch;
                w.log.insert(
                    *commit,
                    CommitTimes {
                        surface: Some(*surface),
                        committed: Some(record.t),
                        ready: (*sync == SyncKind::None).then_some(record.t),
                        ..CommitTimes::default()
                    },
                );
                if let Some(info) = buffer {
                    w.infos.insert(*commit, info.clone());
                }
                (
                    w.detected.is_some(),
                    matches!(w.goal, Goal::Damage),
                    w.wants_buffers(),
                    w.roi,
                )
            };
            if detected {
                return Step::Continue;
            }
            if goal_damage {
                let hit = match roi {
                    None => !damage.is_empty(),
                    Some(roi) => damage_placement(state, *surface, buffer.as_ref())
                        .is_some_and(|p| p.damage_hits(roi, damage)),
                };
                if hit {
                    return detect(state, run, *commit, *surface, record.t);
                }
            } else if goal_pixels && *sync == SyncKind::None {
                return enqueue(state, run, candidate(*commit, *surface, record.t, false));
            }
            Step::Continue
        }
        Event::Ready { surface, commit } => {
            if let Some(step) = early_ready(state, run, *surface, *commit, record.t) {
                return step;
            }
            let (known, pixels, detected) = {
                let mut r = run.borrow_mut();
                let Some(rep) = r.repeat.as_mut() else {
                    return Step::Continue;
                };
                let w = &mut rep.watch;
                let known = match w.log.get_mut(commit) {
                    Some(c) => {
                        c.ready = Some(record.t);
                        true
                    }
                    None => false,
                };
                if w.detected.is_some_and(|(c, _)| c == *commit) {
                    rep.result.ready = Some(record.t);
                }
                (known, w.wants_buffers(), w.detected.is_some())
            };
            if known && pixels && !detected {
                return enqueue(state, run, candidate(*commit, *surface, record.t, false));
            }
            Step::Continue
        }
        Event::AppMark {
            surface,
            commit: Some(commit),
            mark,
        } => {
            let hit = {
                let r = run.borrow();
                let Some(rep) = r.repeat.as_ref() else {
                    return Step::Continue;
                };
                let w = &rep.watch;
                w.detected.is_none()
                    && match (&w.goal, mark) {
                        (Goal::Mark(None), AppMarkKind::InputHandled { input }) => {
                            rep.result.inputs.contains(input)
                        }
                        (Goal::Mark(Some(name)), m) => mark_name(m) == Some(name),
                        _ => false,
                    }
            };
            let surface = surface.or_else(|| {
                run.borrow()
                    .repeat
                    .as_ref()
                    .and_then(|r| r.watch.log.get(commit))
                    .and_then(|c| c.surface)
            });
            if hit && let Some(s) = surface {
                return detect(state, run, *commit, s, record.t);
            }
            Step::Continue
        }
        Event::Latched {
            surface,
            commit,
            output,
            seq,
        } => {
            let latched = model_latch(state, *output, *seq, record.t);
            let mut r = run.borrow_mut();
            let Some(rep) = r.repeat.as_mut() else {
                return Step::Continue;
            };
            if let Some(c) = rep.watch.log.get_mut(commit) {
                c.latched = Some((latched, *seq));
            }
            if let Some((det, det_surface)) = rep.watch.detected
                && (*commit == det || (rep.watch.follow && *surface == det_surface))
            {
                rep.result.latched = Some(latched);
            }
            Step::Continue
        }
        Event::Discarded { commit, .. } => {
            {
                let mut r = run.borrow_mut();
                if let Some(rep) = r.repeat.as_mut() {
                    if let Some(c) = rep.watch.log.get_mut(commit) {
                        c.discarded = true;
                    }
                    if rep.watch.detected.is_some_and(|(c, _)| c == *commit) {
                        rep.watch.follow = true;
                    }
                }
            }
            drain_through(state, run, *commit)
        }
        Event::Presented {
            surface,
            commit,
            seq,
            time,
            ..
        } => {
            let mut r = run.borrow_mut();
            let Some(rep) = r.repeat.as_mut() else {
                return Step::Continue;
            };
            if let Some(c) = rep.watch.log.get_mut(commit) {
                c.presented = Some((*time, *seq));
            }
            let Some((det, det_surface)) = rep.watch.detected else {
                return Step::Continue;
            };
            if *commit != det && !(rep.watch.follow && *surface == det_surface) {
                return Step::Continue;
            }
            take_presentation(rep, *commit, *time, *seq);
            Step::Finished
        }
        _ => Step::Continue,
    }
}

/// Handles a record from before the input. A ready content commit of a target surface becomes
/// the baseline of its surface, and a discarded one goes before its buffer is released.
fn before_the_input(state: &mut State, run: &Shared, record: &Record) -> Step {
    match &record.event {
        Event::Commit {
            surface,
            commit,
            buffer: Some(info),
            sync,
            ..
        } => {
            if !in_target(state, run, *surface) {
                return Step::Continue;
            }
            let ready = {
                let mut r = run.borrow_mut();
                let Some(rep) = r.repeat.as_mut() else {
                    return Step::Continue;
                };
                let w = &mut rep.watch;
                if !w.needs_baselines() || !w.stash.contains_key(commit) {
                    return Step::Continue;
                }
                w.infos.insert(*commit, info.clone());
                if *sync != SyncKind::None {
                    w.early.insert(*commit);
                }
                *sync == SyncKind::None
            };
            if ready {
                return enqueue(state, run, candidate(*commit, *surface, record.t, true));
            }
            Step::Continue
        }
        Event::Ready { surface, commit } => {
            early_ready(state, run, *surface, *commit, record.t).unwrap_or(Step::Continue)
        }
        Event::Discarded { commit, .. } => drain_through(state, run, *commit),
        _ => Step::Continue,
    }
}

/// Queues the baseline readback of a commit from before the input once its fence signals.
/// Returns `None` for any other commit.
fn early_ready(
    state: &mut State,
    run: &Shared,
    surface: SurfaceId,
    commit: CommitId,
    t: Timestamp,
) -> Option<Step> {
    let early = run
        .borrow_mut()
        .repeat
        .as_mut()
        .is_some_and(|rep| rep.watch.early.remove(&commit));
    early.then(|| enqueue(state, run, candidate(commit, surface, t, true)))
}

fn candidate(commit: CommitId, surface: SurfaceId, t: Timestamp, baseline: bool) -> Candidate {
    Candidate {
        commit,
        surface,
        t,
        baseline,
    }
}

/// Returns the latch deadline of the refresh `seq` on a fixed grid, or the record time on other
/// grids. The scheduler takes an update at the deadline, so a late wake leaves the latch at the
/// deadline.
fn model_latch(state: &State, output: OutputId, seq: u64, t: Timestamp) -> Timestamp {
    state
        .outputs
        .get(output)
        .and_then(|o| o.scheduler.latch_deadline(seq))
        .filter(|l| *l <= t)
        .unwrap_or(t)
}

/// Returns the placement of the buffer that a commit shows: its new buffer, or the applied one.
fn damage_placement(
    state: &State,
    surface: SurfaceId,
    info: Option<&BufferInfo>,
) -> Option<BufferPlacement> {
    let origin = surface_origin(state, surface)?;
    match info {
        Some(i) => Some(committed_placement(i, origin)),
        None => {
            let wl = state.surfaces.get(surface)?.wl.clone();
            state.applied_placement(&wl, origin).map(|(_, p)| p)
        }
    }
}
