//! The marks state of one surface in its surface data: whether a marks object exists, and the
//! pending marks that the next commit applies.

use std::sync::Mutex;

use fakeland_types::{InputEventId, Timestamp};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::with_states;

/// Marks that wait for the next commit.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PendingMarks {
    /// The app frame id; the last one wins.
    pub frame: Option<u64>,
    /// Handled input ids without duplicates.
    pub inputs: Vec<InputEventId>,
    /// Spans with begin and end.
    pub spans: Vec<(String, Timestamp, Timestamp)>,
    /// Named marks.
    pub names: Vec<String>,
}

impl PendingMarks {
    /// Returns true when nothing waits.
    pub fn is_empty(&self) -> bool {
        self.frame.is_none()
            && self.inputs.is_empty()
            && self.spans.is_empty()
            && self.names.is_empty()
    }

    /// Adds a handled input id once.
    pub fn add_input(&mut self, id: InputEventId) {
        if !self.inputs.contains(&id) {
            self.inputs.push(id);
        }
    }
}

/// The marks state of a surface.
#[derive(Debug, Default)]
pub struct SlotState {
    /// A marks object exists for the surface.
    pub claimed: bool,
    /// The pending marks.
    pub pending: PendingMarks,
}

#[derive(Debug, Default)]
struct MarksSlot(Mutex<SlotState>);

/// Runs `f` on the marks state of a surface.
pub fn with_slot<R>(surface: &WlSurface, f: impl FnOnce(&mut SlotState) -> R) -> R {
    with_states(surface, |states| {
        states
            .data_map
            .insert_if_missing_threadsafe(MarksSlot::default);
        let slot = states
            .data_map
            .get::<MarksSlot>()
            .expect("the slot was just inserted");
        let mut guard = slot.0.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut guard)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_marks_keep_inputs_once() {
        let mut p = PendingMarks::default();
        assert!(p.is_empty());
        p.add_input(InputEventId(3));
        p.add_input(InputEventId(3));
        p.add_input(InputEventId(4));
        assert_eq!(p.inputs, vec![InputEventId(3), InputEventId(4)]);
        assert!(!p.is_empty());
    }
}
