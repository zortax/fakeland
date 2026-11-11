//! Input handling: each logical input event takes its compositor input id, goes into the log,
//! and waits for the input delay. A due input applies to the scenes, and its id goes into the
//! next frame's input_handled list.

use fakeland_types::{Clock, InputEventId, Nanos};

use super::App;
use crate::input::{GroupKind, InputEffect, InputGroup};
use crate::log::{InputDetail, LogEvent};
use crate::wl::keysym_name;

/// How long an input id may wait for its event before it counts as unmatched.
const MARK_MATCH_WINDOW: Nanos = Nanos::from_millis(100);

impl App {
    /// Closes the open pointer frame and handles it as one input event.
    pub fn finish_pointer_frame(&mut self) {
        if let Some(frame) = self.seat.pointer_frame.frame() {
            self.input_group(frame.kind, frame.effects, frame.wl_time);
        }
    }

    /// Handles one logical input event.
    pub fn input_group(
        &mut self,
        kind: GroupKind,
        effects: Vec<InputEffect>,
        wl_time: Option<u32>,
    ) {
        let received = self.clock.now();
        let marks_id = kind
            .mark_kind()
            .and_then(|k| self.take_input_id(k, received));
        self.log.write(
            received,
            &LogEvent::InputReceived {
                kind,
                wl_time,
                marks_id: marks_id.map(|id| id.0),
                wake: self.wake.0,
                events: effects
                    .iter()
                    .map(|e| InputDetail::of(e, keysym_name))
                    .collect(),
            },
        );
        if effects.is_empty() && marks_id.is_none() {
            return;
        }
        let group = InputGroup {
            kind,
            effects,
            wl_time,
            received,
            wake: self.wake,
            marks_id,
        };
        self.inputs.push(received + self.args.input_delay, group);
    }

    /// Applies every input whose delay passed.
    pub fn apply_due_inputs(&mut self) {
        let window = self.logical_size();
        while let Some(group) = self.inputs.pop_due(self.clock.now()) {
            let mut changed = false;
            for effect in &group.effects {
                changed |= self.scenes.apply(effect, window);
            }
            if let Some(id) = group.marks_id {
                self.marking.handled.push(id);
            }
            if changed || self.args.redraw_on_input {
                self.pacer.mark_dirty();
            }
            let now = self.clock.now();
            self.log.write(
                now,
                &LogEvent::InputApplied {
                    kind: group.kind,
                    marks_id: group.marks_id.map(|id| id.0),
                    changed,
                },
            );
        }
    }

    /// Drops input ids that waited too long for their event.
    pub fn sweep_input_ids(&mut self) {
        let now = self.clock.now();
        let fresh = self.marking.marks.input_events();
        self.marking.matcher.push(now, fresh);
        let cutoff = now - MARK_MATCH_WINDOW;
        for mark in self.marking.matcher.sweep(cutoff) {
            self.log_unmatched(mark);
        }
    }

    fn take_input_id(
        &mut self,
        kind: fakeland_marks::InputKind,
        now: fakeland_types::Timestamp,
    ) -> Option<InputEventId> {
        if !self.marking.marks.is_active() {
            return None;
        }
        let fresh = self.marking.marks.input_events();
        self.marking.matcher.push(now, fresh);
        let matched = self.marking.matcher.take(kind);
        for mark in matched.skipped {
            self.log_unmatched(mark);
        }
        matched.mark.map(|m| m.id)
    }

    fn log_unmatched(&mut self, mark: fakeland_marks::InputMark) {
        let now = self.clock.now();
        self.log.write(
            now,
            &LogEvent::InputUnmatched {
                marks_id: mark.id.0,
                kind: mark.kind.map(|k| format!("{k:?}")),
            },
        );
    }
}
