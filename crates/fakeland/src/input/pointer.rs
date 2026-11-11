//! Pointer injection: absolute motion with hit-tested focus and relative motion, buttons with
//! click-to-focus, and axis events by source. The pointer device model merges motion within one
//! report period and lets a classic wheel send whole detents only.

use fakeland_protocol::server::fakeland_marks_manager_v1::InputKind as MarkKind;
use std::rc::Rc;

use fakeland_scenario::{Button, Position, ScrollSource};
use fakeland_trace::{InputKind, InputSource};
use fakeland_types::{Nanos, Timestamp};
use smithay::backend::input::{Axis, AxisSource, ButtonState};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent, RelativeMotionEvent};
use smithay::utils::{Logical, Point, SERIAL_COUNTER};

use super::{DeferredMotion, InjectError, PendingSent, SentSink};
use crate::config::WheelKind;
use crate::server::State;

/// One axis event of a primitive.
#[derive(Clone, Copy, Debug)]
pub struct AxisInput {
    /// Horizontal distance.
    pub dx: f64,
    /// Vertical distance.
    pub dy: f64,
    /// The source.
    pub source: ScrollSource,
    /// High-resolution wheel steps.
    pub value120: Option<(i32, i32)>,
    /// The event ends the scroll sequence.
    pub stop: bool,
}

impl State {
    /// Moves the pointer to a position. Within one report period of the last report, the motion
    /// waits for the next report slot, and `sink` takes its record when it goes out. A motion
    /// just before its slot goes out at once.
    pub(super) fn inject_motion(
        &mut self,
        pos: &Position,
        source: InputSource,
        sink: Option<&SentSink>,
    ) -> Result<(), InjectError> {
        let (point, _) = self.resolve_position(pos)?;
        let now = self.now();
        if let (Some(period), Some(last)) = (self.input.report_period(), self.input.last_motion)
            && is_early(now, last, period)
        {
            match &mut self.input.deferred {
                Some(d) => {
                    d.point = point;
                    d.source = source;
                    d.sink = sink.cloned();
                }
                None => {
                    let at = last + period;
                    let timer = self.schedule_at(at, State::send_deferred_motion);
                    self.input.deferred = Some(DeferredMotion {
                        point,
                        source,
                        timer,
                        at,
                        sink: sink.cloned(),
                    });
                }
            }
            return Ok(());
        }
        if let Some(d) = self.input.deferred.take() {
            self.cancel_timer(d.timer);
        }
        self.send_motion(point, source)
    }

    /// Sends the deferred motion at its report slot and hands its record to its sink.
    fn send_deferred_motion(&mut self) {
        let Some(d) = self.input.deferred.take() else {
            return;
        };
        let _ = self.send_motion(d.point, d.source);
        let sent = self.close_input_groups();
        if let Some(sink) = d.sink
            && !sent.is_empty()
        {
            sink(self, &sent);
        }
    }

    /// Returns the send time of the deferred motion when `sink` injected it last.
    pub fn deferred_motion_for(&self, sink: &SentSink) -> Option<Timestamp> {
        self.input
            .deferred
            .as_ref()
            .filter(|d| d.sink.as_ref().is_some_and(|s| Rc::ptr_eq(s, sink)))
            .map(|d| d.at)
    }

    /// Sends a deferred motion now, so a later pointer event keeps its order after it. The record
    /// joins the open pointer group.
    pub(super) fn flush_deferred_motion(&mut self) {
        if let Some(d) = self.input.deferred.take() {
            self.cancel_timer(d.timer);
            let _ = self.send_motion(d.point, d.source);
        }
    }

    /// Sends absolute and relative motion to the surface under a global point.
    fn send_motion(
        &mut self,
        point: Point<f64, Logical>,
        source: InputSource,
    ) -> Result<(), InjectError> {
        let pointer = self.seat.get_pointer().ok_or(InjectError::NoPointer)?;
        let old = self.windows.cursor.position;
        let under = self.surface_under(point);
        let (id, surface) =
            self.mark_input(under.as_ref().map(|(s, _)| s), MarkKind::PointerMotion);
        let now = self.now();
        self.windows.cursor.position = point;
        self.input.last_motion = Some(now);
        pointer.motion(
            self,
            under.clone(),
            &MotionEvent {
                location: point,
                serial: SERIAL_COUNTER.next_serial(),
                time: now.wayland_millis(),
            },
        );
        let delta = point - old;
        pointer.relative_motion(
            self,
            under,
            &RelativeMotionEvent {
                delta,
                delta_unaccel: delta,
                utime: now.0 / 1_000,
            },
        );
        self.input.pointer_group.push(PendingSent {
            id,
            kind: InputKind::PointerMotion,
            surface,
            pos: Some((point.x, point.y)),
            detail: String::new(),
            source,
        });
        Ok(())
    }

    /// Presses or releases a button. A press focuses the window under the pointer.
    pub(super) fn inject_button(
        &mut self,
        button: Button,
        pressed: bool,
        source: InputSource,
    ) -> Result<(), InjectError> {
        self.flush_deferred_motion();
        let pointer = self.seat.get_pointer().ok_or(InjectError::NoPointer)?;
        let position = self.windows.cursor.position;
        if pressed
            && let Some(window) = self.window_under(position)
            && self.windows.focus != Some(window)
        {
            self.focus_window(Some(window));
        }
        let focus = pointer.current_focus();
        let (id, surface) = self.mark_input(focus.as_ref(), MarkKind::PointerButton);
        let code = button.code();
        if pressed {
            if !self.input.held_buttons.contains(&code) {
                self.input.held_buttons.push(code);
            }
        } else {
            self.input.held_buttons.retain(|b| *b != code);
        }
        pointer.button(
            self,
            &ButtonEvent {
                serial: SERIAL_COUNTER.next_serial(),
                time: self.now().wayland_millis(),
                button: code,
                state: if pressed {
                    ButtonState::Pressed
                } else {
                    ButtonState::Released
                },
            },
        );
        self.input.pointer_group.push(PendingSent {
            id,
            kind: InputKind::PointerButton,
            surface,
            pos: Some((position.x, position.y)),
            detail: format!(
                "button {code} {}",
                if pressed { "pressed" } else { "released" }
            ),
            source,
        });
        Ok(())
    }

    /// Sends one axis event. A classic wheel holds fractions of a detent back until they add up.
    pub(super) fn inject_axis(
        &mut self,
        axis: AxisInput,
        source: InputSource,
    ) -> Result<(), InjectError> {
        self.flush_deferred_motion();
        let pointer = self.seat.get_pointer().ok_or(InjectError::NoPointer)?;
        let Some(axis) = self.apply_wheel_model(axis) else {
            return Ok(());
        };
        let time = self.now().wayland_millis();
        let mut frame = AxisFrame::new(time).source(match axis.source {
            ScrollSource::Wheel { .. } => AxisSource::Wheel,
            ScrollSource::Finger { .. } => AxisSource::Finger,
            ScrollSource::Continuous => AxisSource::Continuous,
        });
        let (vx, vy) = axis.value120.unwrap_or((0, 0));
        if axis.dx != 0.0 || vx != 0 {
            frame = frame.value(Axis::Horizontal, axis.dx);
            if vx != 0 {
                frame = frame.v120(Axis::Horizontal, vx);
            }
            self.input.scroll_axes.0 = true;
        }
        if axis.dy != 0.0 || vy != 0 {
            frame = frame.value(Axis::Vertical, axis.dy);
            if vy != 0 {
                frame = frame.v120(Axis::Vertical, vy);
            }
            self.input.scroll_axes.1 = true;
        }
        if axis.stop {
            let (h, v) = std::mem::take(&mut self.input.scroll_axes);
            if h {
                frame = frame.stop(Axis::Horizontal);
            }
            if v || !h {
                frame = frame.stop(Axis::Vertical);
            }
        }
        let focus = pointer.current_focus();
        let (id, surface) = self.mark_input(focus.as_ref(), MarkKind::PointerAxis);
        pointer.axis(self, frame);
        let position = self.windows.cursor.position;
        self.input.pointer_group.push(PendingSent {
            id,
            kind: InputKind::PointerAxis,
            surface,
            pos: Some((position.x, position.y)),
            detail: axis_detail(&axis),
            source,
        });
        Ok(())
    }

    /// Applies the wheel kind of the pointer device. Returns `None` when a classic wheel has no
    /// whole detent to send yet.
    fn apply_wheel_model(&mut self, axis: AxisInput) -> Option<AxisInput> {
        let is_wheel = matches!(axis.source, ScrollSource::Wheel { .. });
        let (Some((vx, vy)), true, WheelKind::Discrete) =
            (axis.value120, is_wheel, self.input.device.wheel)
        else {
            return Some(axis);
        };
        let (rx, ry) = self.input.wheel_remainder;
        let (wx, wy) = discrete_steps(rx + vx, ry + vy);
        self.input.wheel_remainder = (rx + vx - wx, ry + vy - wy);
        if wx == 0 && wy == 0 {
            return None;
        }
        let per = |d: f64, v: i32| if v == 0 { 0.0 } else { d / v as f64 };
        Some(AxisInput {
            dx: per(axis.dx, vx) * wx as f64,
            dy: per(axis.dy, vy) * wy as f64,
            value120: Some((wx, wy)),
            ..axis
        })
    }
}

/// The largest span by which a motion can precede its report slot and still count as on time.
const MAX_SLOT_TOLERANCE: Nanos = Nanos::from_micros(50);

/// Returns true when a motion at `now` comes early for the report slot after `last`. A motion
/// within a tenth of a period of its slot, and at most [`MAX_SLOT_TOLERANCE`] before it, counts
/// as on time, so timer jitter never defers evenly spaced motion.
pub fn is_early(now: Timestamp, last: Timestamp, period: Nanos) -> bool {
    now + (period / 10).min(MAX_SLOT_TOLERANCE) < last + period
}

/// Returns the whole detents of accumulated `value120` steps, as `value120`.
pub fn discrete_steps(x: i32, y: i32) -> (i32, i32) {
    ((x / 120) * 120, (y / 120) * 120)
}

fn axis_detail(a: &AxisInput) -> String {
    let source = match a.source {
        ScrollSource::Wheel { .. } => "wheel",
        ScrollSource::Finger { .. } => "finger",
        ScrollSource::Continuous => "continuous",
    };
    let mut s = format!("{source} dx={} dy={}", a.dx, a.dy);
    if let Some((x, y)) = a.value120 {
        s.push_str(&format!(" v120=({x},{y})"));
    }
    if a.stop {
        s.push_str(" stop");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_on_its_slot_is_on_time() {
        let period = Nanos::from_millis(8);
        let last = Timestamp(1_000_000_000);
        assert!(is_early(last + Nanos::from_millis(1), last, period));
        assert!(is_early(last + Nanos::from_micros(7_900), last, period));
        assert!(!is_early(last + Nanos::from_micros(7_960), last, period));
        assert!(!is_early(last + period, last, period));
        assert!(!is_early(last + Nanos::from_millis(9), last, period));
        let fast = Nanos::from_micros(125);
        assert!(is_early(last + Nanos::from_micros(110), last, fast));
        assert!(!is_early(last + Nanos::from_micros(115), last, fast));
    }

    #[test]
    fn discrete_steps_keep_whole_detents() {
        assert_eq!(discrete_steps(60, 0), (0, 0));
        assert_eq!(discrete_steps(130, -250), (120, -240));
        assert_eq!(discrete_steps(-119, 360), (0, 360));
    }
}
