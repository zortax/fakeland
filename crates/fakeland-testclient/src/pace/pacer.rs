//! The present-mode state machine. It decides when the client renders and commits a frame:
//!
//! - `callback`: on each frame callback, continuously.
//! - `on-demand`: when the picture changed, throttled by frame callbacks.
//! - `unthrottled`: as fast as buffers allow, ignoring frame callbacks.
//! - `fixed-rate`: on its own tick grid.
//!
//! The idle scene renders only for configures in every mode. An urgent frame, which answers a
//! configure, skips the wait for a frame callback. Buffer availability is the caller's concern.

use fakeland_types::{Hz, Timestamp};
use serde::Serialize;

use super::Ticks;

/// When the client commits frames.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "kebab-case")]
pub enum PresentMode {
    /// Render on each frame callback.
    Callback,
    /// Commit as fast as possible and ignore frame callbacks.
    Unthrottled,
    /// Commit on a timer at a fixed rate.
    FixedRate,
    /// Render only when the picture changes, throttled by frame callbacks.
    OnDemand,
}

/// The decision of one poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decision {
    /// Render a frame now.
    Render,
    /// Wait for an event or for the deadline.
    Wait,
}

/// The pacing state.
#[derive(Clone, Debug)]
pub struct Pacer {
    mode: PresentMode,
    rate: Option<Hz>,
    continuous: bool,
    ready: bool,
    gate: Option<Timestamp>,
    dirty: bool,
    urgent: bool,
    callback_pending: bool,
    ticks: Option<Ticks>,
    skipped_ticks: u64,
    stopped: bool,
}

impl Pacer {
    /// Builds a pacer. `continuous` is true when the picture must be redrawn without changes:
    /// every mode but on-demand redraws continuously unless the scene is idle, and on-demand
    /// redraws continuously while an animation runs. `rate` is required for fixed-rate.
    pub fn new(mode: PresentMode, rate: Option<Hz>, idle: bool, animating: bool) -> Self {
        let continuous = !idle && (mode != PresentMode::OnDemand || animating);
        Self {
            mode,
            rate,
            continuous,
            ready: false,
            gate: None,
            dirty: false,
            urgent: false,
            callback_pending: false,
            ticks: None,
            skipped_ticks: 0,
            stopped: false,
        }
    }

    /// Returns true when the mode waits for frame callbacks.
    pub fn throttled(&self) -> bool {
        matches!(self.mode, PresentMode::Callback | PresentMode::OnDemand)
    }

    /// Returns the number of fixed-rate ticks that passed without a frame.
    #[cfg(test)]
    pub fn skipped_ticks(&self) -> u64 {
        self.skipped_ticks
    }

    /// Allows rendering from `start` on. The first frame renders at `start`.
    pub fn start(&mut self, start: Timestamp) {
        self.ready = true;
        self.dirty = true;
        self.gate = Some(start);
        if self.mode == PresentMode::FixedRate {
            let rate = self.rate.expect("fixed-rate needs a rate");
            self.ticks = Some(Ticks::new(start, rate));
        }
    }

    /// Stops all rendering.
    pub fn stop(&mut self) {
        self.stopped = true;
    }

    /// Records that the picture changed.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Records that a frame must answer a configure as soon as possible.
    pub fn mark_urgent(&mut self) {
        self.dirty = true;
        self.urgent = true;
    }

    /// Records the arrival of the frame callback.
    pub fn callback_done(&mut self) {
        self.callback_pending = false;
    }

    /// Decides whether to render at `now`. A fixed-rate tick that passes without a reason to
    /// render is consumed.
    pub fn poll(&mut self, now: Timestamp) -> Decision {
        if !self.ready || self.stopped || self.gate.is_some_and(|g| now < g) {
            return Decision::Wait;
        }
        self.gate = None;
        let wanted = self.dirty || self.continuous;
        let render = match self.mode {
            PresentMode::Callback | PresentMode::OnDemand => {
                wanted && (!self.callback_pending || self.urgent)
            }
            PresentMode::Unthrottled => wanted,
            PresentMode::FixedRate => {
                let ticks = self.ticks.as_mut().expect("started");
                if !ticks.due(now) {
                    false
                } else if wanted {
                    true
                } else {
                    ticks.advance(now);
                    false
                }
            }
        };
        if render {
            Decision::Render
        } else {
            Decision::Wait
        }
    }

    /// Records a rendered frame. `requested_callback` tells whether the commit carries a frame
    /// callback request. Returns the number of skipped fixed-rate ticks.
    pub fn rendered(&mut self, now: Timestamp, requested_callback: bool) -> u64 {
        self.dirty = false;
        self.urgent = false;
        if requested_callback {
            self.callback_pending = true;
        }
        let skipped = self.ticks.as_mut().map_or(0, |t| t.advance(now));
        self.skipped_ticks += skipped;
        skipped
    }

    /// Returns the next time at which `poll` can change its answer without an event.
    pub fn deadline(&self) -> Option<Timestamp> {
        if !self.ready || self.stopped {
            return None;
        }
        if let Some(gate) = self.gate {
            return Some(match &self.ticks {
                Some(t) => t.next().max(gate),
                None => gate,
            });
        }
        self.ticks.as_ref().map(Ticks::next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: Timestamp = Timestamp(1_000_000_000);

    #[test]
    fn nothing_before_start_or_gate() {
        let mut p = Pacer::new(PresentMode::Callback, None, false, false);
        assert_eq!(p.poll(T0), Decision::Wait);
        p.start(Timestamp(T0.0 + 500));
        assert_eq!(p.poll(T0), Decision::Wait);
        assert_eq!(p.deadline(), Some(Timestamp(T0.0 + 500)));
        assert_eq!(p.poll(Timestamp(T0.0 + 500)), Decision::Render);
    }

    #[test]
    fn callback_mode_renders_once_per_callback() {
        let mut p = Pacer::new(PresentMode::Callback, None, false, false);
        p.start(T0);
        assert_eq!(p.poll(T0), Decision::Render);
        p.rendered(T0, true);
        assert_eq!(p.poll(T0), Decision::Wait);
        p.callback_done();
        assert_eq!(p.poll(T0), Decision::Render);
    }

    #[test]
    fn on_demand_renders_only_changes() {
        let mut p = Pacer::new(PresentMode::OnDemand, None, false, false);
        p.start(T0);
        assert_eq!(p.poll(T0), Decision::Render);
        p.rendered(T0, true);
        p.callback_done();
        assert_eq!(p.poll(T0), Decision::Wait);
        p.mark_dirty();
        assert_eq!(p.poll(T0), Decision::Render);
        p.rendered(T0, true);
        // A change while the callback is pending waits for the callback.
        p.mark_dirty();
        assert_eq!(p.poll(T0), Decision::Wait);
        p.callback_done();
        assert_eq!(p.poll(T0), Decision::Render);
    }

    #[test]
    fn on_demand_animation_is_continuous() {
        let mut p = Pacer::new(PresentMode::OnDemand, None, false, true);
        p.start(T0);
        p.rendered(T0, true);
        p.callback_done();
        assert_eq!(p.poll(T0), Decision::Render);
    }

    #[test]
    fn urgent_frames_skip_the_callback_wait() {
        let mut p = Pacer::new(PresentMode::Callback, None, false, false);
        p.start(T0);
        p.rendered(T0, true);
        p.mark_urgent();
        assert_eq!(p.poll(T0), Decision::Render);
        p.rendered(T0, true);
        assert_eq!(p.poll(T0), Decision::Wait);
    }

    #[test]
    fn idle_renders_once() {
        for mode in [
            PresentMode::Callback,
            PresentMode::Unthrottled,
            PresentMode::OnDemand,
        ] {
            let mut p = Pacer::new(mode, None, true, false);
            p.start(T0);
            assert_eq!(p.poll(T0), Decision::Render, "{mode:?}");
            p.rendered(T0, mode != PresentMode::Unthrottled);
            p.callback_done();
            assert_eq!(p.poll(T0), Decision::Wait, "{mode:?}");
            p.mark_urgent();
            assert_eq!(p.poll(T0), Decision::Render, "{mode:?}");
        }
    }

    #[test]
    fn unthrottled_ignores_callbacks() {
        let mut p = Pacer::new(PresentMode::Unthrottled, None, false, false);
        p.start(T0);
        p.rendered(T0, true);
        assert_eq!(p.poll(T0), Decision::Render);
        assert!(!p.throttled());
    }

    #[test]
    fn fixed_rate_follows_ticks() {
        let mut p = Pacer::new(PresentMode::FixedRate, Some(Hz(100.0)), false, false);
        p.start(T0);
        assert_eq!(p.poll(T0), Decision::Render);
        assert_eq!(p.rendered(T0, false), 0);
        let t1 = Timestamp(T0.0 + 10_000_000);
        assert_eq!(p.deadline(), Some(t1));
        assert_eq!(p.poll(Timestamp(t1.0 - 1)), Decision::Wait);
        assert_eq!(p.poll(t1), Decision::Render);
        // The frame renders late, after ticks 2 and 3 passed.
        let late = Timestamp(T0.0 + 35_000_000);
        assert_eq!(p.rendered(late, false), 2);
        assert_eq!(p.deadline(), Some(Timestamp(T0.0 + 40_000_000)));
        assert_eq!(p.skipped_ticks(), 2);
    }

    #[test]
    fn fixed_rate_idle_consumes_unused_ticks() {
        let mut p = Pacer::new(PresentMode::FixedRate, Some(Hz(100.0)), true, false);
        p.start(T0);
        p.poll(T0);
        p.rendered(T0, false);
        let t1 = Timestamp(T0.0 + 10_000_000);
        assert_eq!(p.poll(t1), Decision::Wait);
        assert_eq!(p.deadline(), Some(Timestamp(T0.0 + 20_000_000)));
        p.mark_dirty();
        assert_eq!(p.poll(Timestamp(T0.0 + 15_000_000)), Decision::Wait);
        assert_eq!(p.poll(Timestamp(T0.0 + 20_000_000)), Decision::Render);
    }

    #[test]
    fn stop_ends_rendering() {
        let mut p = Pacer::new(PresentMode::Unthrottled, None, false, false);
        p.start(T0);
        p.stop();
        assert_eq!(p.poll(T0), Decision::Wait);
        assert_eq!(p.deadline(), None);
    }
}
