//! The tree walk: a breadth-first walk with bounded concurrency per level, a depth limit, a node
//! budget and an optional visibility filter. Children of an app root are paired with compositor
//! windows once their level is fetched; a paired frame calibrates the coordinate mapping of its
//! subtree.

use std::collections::HashMap;

use fakeland_types::ProcessId;
use futures_util::StreamExt;
use futures_util::stream;

use super::A11y;
use super::fetch::{FetchSpec, Fetched, fetch};
use crate::matching::{FrameCandidate, match_frames};
use crate::{CoordSpace, Extents, Mapping, Node, NodeRef, TreeOptions, WindowFrame, roles};

/// Where a queued node sits relative to the compositor windows.
#[derive(Clone, Debug)]
pub(crate) enum Place {
    /// No window is known.
    Unknown,
    /// A child of an app root. The pairing decides once its level is fetched.
    AppChild(Option<ProcessId>),
    /// The frame of a window. Its extents calibrate the mapping.
    Frame(WindowFrame),
    /// A node inside a mapped window.
    Inside(Mapping),
}

struct Item {
    id: NodeRef,
    parent: Option<usize>,
    depth: usize,
    place: Place,
    space: CoordSpace,
    parent_extents: Option<Extents>,
}

impl A11y {
    /// Walks the tree from a root at a known place.
    pub(crate) async fn walk(
        &self,
        root: NodeRef,
        place: Place,
        space: CoordSpace,
        parent_extents: Option<Extents>,
        windows: &[WindowFrame],
        opts: &TreeOptions,
    ) -> Node {
        let max_nodes = opts.max_nodes.max(1);
        let mut nodes: Vec<Option<Node>> = Vec::new();
        let mut kids: Vec<Vec<usize>> = Vec::new();
        let mut reserved = 1usize;
        let mut pids: HashMap<String, Option<ProcessId>> = HashMap::new();
        let mut frontier = vec![Item {
            id: root,
            parent: None,
            depth: 0,
            place,
            space,
            parent_extents,
        }];
        while !frontier.is_empty() {
            let jobs: Vec<_> = frontier
                .iter()
                .map(|item| {
                    let spec = FetchSpec {
                        space: item.space,
                        text_limit: opts.text_limit,
                        children: item.depth < opts.max_depth,
                    };
                    fetch(&self.conn, item.id.clone(), spec)
                })
                .collect();
            let fetched: Vec<Fetched> = stream::iter(jobs)
                .buffered(opts.concurrency.max(1))
                .collect()
                .await;
            let places = pair_app_children(&frontier, &fetched, windows);
            let mut next = Vec::new();
            for ((item, f), place) in frontier.into_iter().zip(fetched).zip(places) {
                let Fetched { mut node, children } = f;
                let is_app = roles::normalize(&node.role) == "application";
                if opts.showing_only
                    && item.depth > 0
                    && !is_app
                    && node.error.is_none()
                    && !node.has_state("showing")
                {
                    if let Some(p) = item.parent
                        && let Some(Some(parent)) = nodes.get_mut(p)
                    {
                        parent.omitted += 1;
                    }
                    continue;
                }
                let (child_place, child_space) =
                    settle(&mut node, place, item.space, item.parent_extents);
                let child_place = if is_app {
                    let pid = match pids.get(&node.id.bus) {
                        Some(p) => *p,
                        None => {
                            let p = self.pid_of(&node.id.bus).await;
                            pids.insert(node.id.bus.clone(), p);
                            p
                        }
                    };
                    Place::AppChild(pid)
                } else {
                    child_place
                };
                if item.depth >= opts.max_depth {
                    node.omitted = node.child_count;
                }
                let idx = nodes.len();
                let node_extents = node.extents;
                let room = max_nodes.saturating_sub(reserved);
                let take = children.len().min(room);
                node.omitted += children.len() - take;
                reserved += take;
                nodes.push(Some(node));
                kids.push(Vec::new());
                if let Some(p) = item.parent {
                    kids[p].push(idx);
                }
                for c in children.into_iter().take(take) {
                    next.push(Item {
                        id: c,
                        parent: Some(idx),
                        depth: item.depth + 1,
                        place: child_place.clone(),
                        space: child_space,
                        parent_extents: node_extents,
                    });
                }
            }
            frontier = next;
        }
        assemble(0, &mut nodes, &kids)
    }
}

/// Pairs the fetched children of each app root with windows and returns the place of every item.
fn pair_app_children(
    frontier: &[Item],
    fetched: &[Fetched],
    windows: &[WindowFrame],
) -> Vec<Place> {
    let mut places: Vec<Place> = frontier.iter().map(|i| i.place.clone()).collect();
    let mut groups: HashMap<usize, (Option<ProcessId>, Vec<usize>)> = HashMap::new();
    for (k, item) in frontier.iter().enumerate() {
        if let (Place::AppChild(pid), Some(p)) = (&item.place, item.parent) {
            groups.entry(p).or_insert((*pid, Vec::new())).1.push(k);
        }
    }
    for (_, (pid, members)) in groups {
        let candidates: Vec<FrameCandidate> = members
            .iter()
            .map(|&k| {
                let n = &fetched[k].node;
                FrameCandidate {
                    name: n.name.clone(),
                    active: n.has_state("active"),
                    showing: n.has_state("showing"),
                }
            })
            .collect();
        let pairs = match pid {
            Some(p) => match_frames(Some(p), &candidates, windows),
            None => vec![None; members.len()],
        };
        for (&k, w) in members.iter().zip(pairs) {
            places[k] = match w {
                Some(w) => Place::Frame(windows[w].clone()),
                None => Place::Unknown,
            };
        }
    }
    places
}

/// Sets the window and bounds of a fetched node and returns the place and space of its children.
pub(crate) fn settle(
    node: &mut Node,
    place: Place,
    space: CoordSpace,
    parent_extents: Option<Extents>,
) -> (Place, CoordSpace) {
    match place {
        Place::Frame(w) => {
            let space = node.extents.map(|e| e.space).unwrap_or(space);
            let m = Mapping::calibrate(node.extents, &w, space);
            node.window = Some(w.id);
            node.bounds = node.extents.map(|e| m.map(&e));
            (Place::Inside(m), space)
        }
        Place::Inside(m) => {
            let m = m.descend(parent_extents, node.extents);
            node.window = Some(m.window);
            node.bounds = node.extents.map(|e| m.map(&e));
            (Place::Inside(m), space)
        }
        Place::Unknown | Place::AppChild(_) => (Place::Unknown, space),
    }
}

fn assemble(idx: usize, nodes: &mut [Option<Node>], kids: &[Vec<usize>]) -> Node {
    let mut node = nodes[idx]
        .take()
        .unwrap_or_else(|| Node::empty(NodeRef::new("", "/")));
    node.children = kids[idx]
        .iter()
        .map(|&c| assemble(c, nodes, kids))
        .collect();
    node
}
