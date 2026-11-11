//! Pairing of accessible frames with compositor windows. A frame belongs to a window of the same
//! process. The title decides first, then the active state against keyboard focus, then a single
//! remaining pair.

use fakeland_types::ProcessId;

use crate::WindowFrame;

/// What the pairing reads from a frame node.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameCandidate {
    /// The accessible name.
    pub name: String,
    /// The frame carries the `active` state.
    pub active: bool,
    /// The frame carries the `showing` state.
    pub showing: bool,
}

/// Returns the window index of each frame of an app with pid `pid`. `None` as pid allows every
/// window.
pub fn match_frames(
    pid: Option<ProcessId>,
    frames: &[FrameCandidate],
    windows: &[WindowFrame],
) -> Vec<Option<usize>> {
    let mut free: Vec<usize> = (0..windows.len())
        .filter(|&i| pid.is_none_or(|p| windows[i].owned_by(p)))
        .collect();
    let mut out = vec![None; frames.len()];
    let take = |free: &mut Vec<usize>, w: usize| free.retain(|&x| x != w);

    for (fi, f) in frames.iter().enumerate() {
        if f.name.is_empty() {
            continue;
        }
        if let Some(&w) = free.iter().find(|&&w| windows[w].title == f.name) {
            out[fi] = Some(w);
            take(&mut free, w);
        }
    }
    for (fi, f) in frames.iter().enumerate() {
        if out[fi].is_some() || !f.active {
            continue;
        }
        if let Some(&w) = free.iter().find(|&&w| windows[w].focused) {
            out[fi] = Some(w);
            take(&mut free, w);
        }
    }
    let open: Vec<usize> = (0..frames.len()).filter(|&i| out[i].is_none()).collect();
    let showing: Vec<usize> = open
        .iter()
        .copied()
        .filter(|&i| frames[i].showing)
        .collect();
    let pool = if showing.is_empty() { open } else { showing };
    if pool.len() == 1 && free.len() == 1 {
        out[pool[0]] = Some(free[0]);
    }
    out
}

#[cfg(test)]
mod tests {
    use fakeland_types::{Rect, WindowId};

    use super::*;

    fn win(id: u64, pid: u32, title: &str, focused: bool) -> WindowFrame {
        WindowFrame {
            id: WindowId(id),
            pid: Some(ProcessId(pid)),
            app: Some(ProcessId(pid)),
            title: title.into(),
            app_id: String::new(),
            geometry: Rect::new(0, 0, 10, 10),
            surface: Rect::new(0, 0, 10, 10),
            scale: 1.0,
            focused,
        }
    }

    fn frame(name: &str, active: bool, showing: bool) -> FrameCandidate {
        FrameCandidate {
            name: name.into(),
            active,
            showing,
        }
    }

    #[test]
    fn titles_pair_first() {
        let windows = [
            win(1, 5, "A", false),
            win(2, 5, "B", true),
            win(3, 6, "A", false),
        ];
        let frames = [frame("B", false, true), frame("A", true, true)];
        assert_eq!(
            match_frames(Some(ProcessId(5)), &frames, &windows),
            vec![Some(1), Some(0)]
        );
    }

    #[test]
    fn active_frame_takes_the_focused_window() {
        let windows = [
            win(1, 5, "x", false),
            win(2, 5, "y", true),
            win(3, 5, "z", false),
        ];
        let frames = [frame("", false, true), frame("", true, true)];
        assert_eq!(
            match_frames(Some(ProcessId(5)), &frames, &windows),
            vec![None, Some(1)]
        );
        let two = &windows[..2];
        assert_eq!(
            match_frames(Some(ProcessId(5)), &frames, two),
            vec![Some(0), Some(1)]
        );
    }

    #[test]
    fn single_showing_frame_takes_the_last_window() {
        let windows = [win(1, 5, "Window", false)];
        let frames = [frame("Other", false, false), frame("Untitled", false, true)];
        assert_eq!(
            match_frames(Some(ProcessId(5)), &frames, &windows),
            vec![None, Some(0)]
        );
    }

    #[test]
    fn other_processes_never_pair() {
        let windows = [win(1, 6, "A", true)];
        let frames = [frame("A", true, true)];
        assert_eq!(
            match_frames(Some(ProcessId(5)), &frames, &windows),
            vec![None]
        );
        assert_eq!(match_frames(None, &frames, &windows), vec![Some(0)]);
    }
}
