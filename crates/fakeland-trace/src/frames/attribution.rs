//! Input attribution: the commit that handles each input event and the presentation that shows it.

use std::collections::HashMap;

use fakeland_types::{CommitId, InputEventId, SurfaceId, Timestamp};

use crate::frames::{Attribution, FrameTimeline};

/// Fills the handling commit and presentation of every input event.
///
/// An `input_handled` mark names the handling commit exactly. Without a mark, the handling commit
/// is the first commit with a new buffer in the surface tree of the receiving surface at or after
/// the send time.
pub(crate) fn attribute(tl: &mut FrameTimeline) {
    let mut by_mark: HashMap<InputEventId, CommitId> = HashMap::new();
    let mut by_root: HashMap<SurfaceId, Vec<(Timestamp, CommitId)>> = HashMap::new();
    for f in tl.frames.values().flatten() {
        for input in &f.marks.inputs_handled {
            by_mark.entry(*input).or_insert(f.commit);
        }
        if f.buffer.is_some() && !f.detached {
            by_root
                .entry(tl.tree.root(f.surface))
                .or_default()
                .push((f.committed, f.commit));
        }
    }
    for commits in by_root.values_mut() {
        commits.sort();
    }
    let mut found = Vec::with_capacity(tl.inputs.len());
    for input in &tl.inputs {
        let handled = if let Some(c) = by_mark.get(&input.id) {
            Some((*c, Attribution::Mark))
        } else {
            input.surface.and_then(|s| {
                let commits = by_root.get(&tl.tree.root(s))?;
                let i = commits.partition_point(|(t, _)| *t < input.sent);
                commits.get(i).map(|(_, c)| (*c, Attribution::NextCommit))
            })
        };
        found.push(handled);
    }
    for (i, handled) in found.into_iter().enumerate() {
        let Some((commit, how)) = handled else {
            continue;
        };
        let handled_at = tl.frame(commit).map(|f| f.committed);
        let shown = shown_by(tl, commit);
        let input = &mut tl.inputs[i];
        input.handled_by = Some(commit);
        input.attribution = how;
        input.handled_at = handled_at;
        input.presented_commit = shown.map(|(c, _)| c);
        input.presented = shown.map(|(_, t)| t);
    }
}

/// Returns the commit and time of the presentation that first shows a commit: the commit itself,
/// or the next presented frame of its surface.
fn shown_by(tl: &FrameTimeline, commit: CommitId) -> Option<(CommitId, Timestamp)> {
    let (surface, index) = tl.index.get(&commit)?;
    tl.frames(*surface)[*index..]
        .iter()
        .filter(|f| f.commit == commit || f.is_presented_frame())
        .find_map(|f| f.presented.map(|p| (f.commit, p.time)))
}
