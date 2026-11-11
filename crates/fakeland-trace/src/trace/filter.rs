//! Filters that narrow a trace to surfaces or windows.

use std::collections::{BTreeSet, HashMap};

use fakeland_types::{CommitId, SurfaceId, WindowId};

use crate::event::Record;
use crate::trace::{SurfaceTree, Trace};

impl Trace {
    /// Returns the records that concern any of the given surfaces, plus records that name no
    /// surface and no window (vblanks, outputs, processes, samples, marks and scenarios).
    pub fn for_surfaces(&self, surfaces: &BTreeSet<SurfaceId>) -> Trace {
        let tree = SurfaceTree::from_records(&self.records);
        let commits: HashMap<CommitId, SurfaceId> = self
            .records
            .iter()
            .filter_map(|r| Some((r.event.commit()?, r.event.surface()?)))
            .collect();
        self.filter(|r| keeps(r, surfaces, &tree, &commits))
    }

    /// Returns the records of one surface. See [`Trace::for_surfaces`].
    pub fn for_surface(&self, surface: SurfaceId) -> Trace {
        self.for_surfaces(&BTreeSet::from([surface]))
    }

    /// Returns the records of a window: its toplevel surface, its subsurfaces and popups, and the
    /// global records. See [`Trace::for_surfaces`].
    pub fn for_window(&self, window: WindowId) -> Trace {
        let tree = SurfaceTree::from_records(&self.records);
        self.for_surfaces(&tree.window_surfaces(window))
    }
}

fn keeps(
    r: &Record,
    surfaces: &BTreeSet<SurfaceId>,
    tree: &SurfaceTree,
    commits: &HashMap<CommitId, SurfaceId>,
) -> bool {
    if let Some(s) = r.event.surface() {
        return surfaces.contains(&s);
    }
    if let Some(w) = r.event.window() {
        return tree
            .window_surface(w)
            .is_some_and(|s| surfaces.contains(&s));
    }
    if let Some(c) = r.event.commit() {
        return commits.get(&c).is_some_and(|s| surfaces.contains(s));
    }
    true
}

#[cfg(test)]
mod tests {
    use fakeland_types::{ProbeId, Timestamp};

    use super::*;
    use crate::event::{Environment, Event, SyncKind};
    use crate::trace::TraceHeader;

    fn commit(s: u64, c: u64) -> Event {
        Event::Commit {
            surface: SurfaceId(s),
            commit: CommitId(c),
            buffer: None,
            damage: vec![],
            frame_callback: false,
            presentation_feedback: false,
            sync: SyncKind::None,
            fifo_barrier: false,
            fifo_wait: false,
            target: None,
            configure_serial: None,
            detached: false,
        }
    }

    #[test]
    fn window_filter_keeps_its_tree_and_globals() {
        let mut t = Trace::new(TraceHeader::new(
            Environment::default(),
            vec![],
            Timestamp(0),
        ));
        t.push(
            Timestamp(1),
            Event::WindowMapped {
                window: WindowId(1),
                surface: SurfaceId(1),
                app_id: "a".into(),
                title: String::new(),
                output: None,
            },
        );
        t.push(
            Timestamp(1),
            Event::SurfaceRole {
                surface: SurfaceId(2),
                role: "subsurface".into(),
                parent: Some(SurfaceId(1)),
            },
        );
        t.push(Timestamp(2), commit(1, 1));
        t.push(Timestamp(2), commit(2, 2));
        t.push(Timestamp(2), commit(3, 3));
        t.push(
            Timestamp(3),
            Event::ProbeDetected {
                probe: ProbeId(1),
                commit: CommitId(3),
                detector: "any_damage".into(),
            },
        );
        t.push(Timestamp(4), Event::UserMark { label: "x".into() });
        let w = t.for_window(WindowId(1));
        let names: Vec<_> = w.records.iter().map(|r| r.event.name()).collect();
        assert_eq!(
            names,
            [
                "window_mapped",
                "surface_role",
                "commit",
                "commit",
                "user_mark"
            ]
        );
        assert_eq!(t.for_surface(SurfaceId(3)).records.len(), 3);
    }
}
