//! Pixel tests of candidate buffers and the record of a detection. A detection follows its
//! commit to the presentation, or finishes at once when the commit is already on screen.

use fakeland_trace::Event;
use fakeland_types::{CommitId, SurfaceId, Timestamp};

use super::queue::Candidate;
use super::readback::committed_placement;
use super::run::{CommitLog, Repeat, Shared};
use crate::server::State;
use crate::waits::changed_fraction;

use super::detect::{Goal, Step, surface_origin};

/// Reads back the region of a candidate buffer. A candidate from before the input becomes the
/// baseline of its surface; any other candidate goes through the pixel detector.
pub(super) fn test_candidate(state: &mut State, run: &Shared, c: Candidate) -> Step {
    let Candidate {
        commit,
        surface,
        t,
        baseline: is_baseline,
    } = c;
    let (buffer, info, roi, baseline) = {
        let mut r = run.borrow_mut();
        let Some(rep) = r.repeat.as_mut().filter(|rep| !rep.finished) else {
            return Step::Continue;
        };
        let w = &mut rep.watch;
        if w.detected.is_some() {
            return Step::Continue;
        }
        let Some(buffer) = w.stash.remove(&commit) else {
            return Step::Continue;
        };
        let Some(info) = w.infos.get(&commit).cloned() else {
            return Step::Continue;
        };
        let Some(roi) = w.roi else {
            return Step::Continue;
        };
        (buffer, info, roi, w.baselines.get(&surface).cloned())
    };
    let Some(origin) = surface_origin(state, surface) else {
        return Step::Continue;
    };
    let placement = committed_placement(&info, origin);
    let Some(read) = state.read_region(&buffer, &placement, roi) else {
        return Step::Continue;
    };
    let hit = {
        let mut r = run.borrow_mut();
        r.readback_max = r.readback_max.max(read.cost);
        let Some(rep) = r.repeat.as_mut() else {
            return Step::Continue;
        };
        rep.result.readbacks += 1;
        rep.result.readback_cost += read.cost;
        if is_baseline {
            rep.watch.baselines.insert(surface, read.image);
            return Step::Continue;
        }
        match &rep.watch.goal {
            Goal::Changed(threshold) => match &baseline {
                Some(base) => changed_fraction(&read.image, base).unwrap_or(1.0) > *threshold,
                None => true,
            },
            Goal::Match(cond) => cond
                .matches(&read.image, baseline.as_ref(), read.scale)
                .unwrap_or(false),
            _ => false,
        }
    };
    if hit {
        return detect(state, run, commit, surface, t);
    }
    Step::Continue
}

/// Records the detected commit and starts following it to its presentation. Returns
/// [`Step::Finished`] when the response is already on screen: a pixel readback can run after
/// the presentation of its commit.
pub(super) fn detect(
    state: &mut State,
    run: &Shared,
    commit: CommitId,
    surface: SurfaceId,
    t: Timestamp,
) -> Step {
    let (probe, detector, step) = {
        let mut r = run.borrow_mut();
        let detector = r.report.detector.clone();
        let Some(rep) = r.repeat.as_mut() else {
            return Step::Continue;
        };
        if rep.watch.detected.is_some() {
            return Step::Continue;
        }
        rep.watch.detected = Some((commit, surface));
        rep.watch.queue.clear();
        let times = rep.watch.log.get(&commit).cloned().unwrap_or_default();
        let res = &mut rep.result;
        res.detected_commit = Some(commit);
        res.surface = Some(surface);
        res.detected_at = Some(t);
        res.committed = times.committed;
        res.ready = times.ready;
        res.latched = times.latched.map(|(t, _)| t);
        let probe = res.probe;
        rep.watch.follow = times.discarded;
        let shown = if times.discarded {
            next_presented(&rep.watch.log, surface, times.committed)
        } else {
            times.presented.map(|(time, seq)| (commit, time, seq))
        };
        let step = match shown {
            Some((c, time, seq)) => {
                take_presentation(rep, c, time, seq);
                Step::Finished
            }
            None => Step::Continue,
        };
        (probe, detector, step)
    };
    if let Some(probe) = probe {
        state.emit(Event::ProbeDetected {
            probe,
            commit,
            detector,
        });
    }
    step
}

/// Stores the presentation of the response in the result.
pub(super) fn take_presentation(rep: &mut Repeat, commit: CommitId, time: Timestamp, seq: u64) {
    rep.result.presented = Some(time);
    rep.result.presented_commit = Some(commit);
    rep.result.presented_seq = Some(seq);
    if let Some((t, _)) = rep.watch.log.get(&commit).and_then(|c| c.latched) {
        rep.result.latched = Some(t);
    }
}

/// Returns the first logged presentation of a surface at or after a commit time.
fn next_presented(
    log: &CommitLog,
    surface: SurfaceId,
    after: Option<Timestamp>,
) -> Option<(CommitId, Timestamp, u64)> {
    log.iter()
        .filter(|(_, c)| c.surface == Some(surface))
        .filter(|(_, c)| after.is_none_or(|a| c.committed.is_some_and(|t| t >= a)))
        .filter_map(|(id, c)| c.presented.map(|(time, seq)| (*id, time, seq)))
        .min_by_key(|(_, time, seq)| (*time, *seq))
}
