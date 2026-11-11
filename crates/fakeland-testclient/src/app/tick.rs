//! One pass of client work after each dispatch: exit checks, configure acks, due inputs, input
//! id sweeps and at most one frame. It also computes the next deadline for the loop timer.

use fakeland_types::{Clock, Nanos, Timestamp};

use super::{App, ExitReason};
use crate::pace::Decision;

/// How often the loop wakes up while input ids wait for their event.
const SWEEP_INTERVAL: Nanos = Nanos::from_millis(50);

impl App {
    /// Runs the work that is due now.
    pub fn tick(&mut self) {
        let now = self.clock.now();
        if self.exit_at.is_some_and(|t| now >= t) {
            self.request_exit(ExitReason::Timeout);
        }
        if let Some(until) = self.drain_until
            && (self.render.feedback_pending == 0 || now >= until)
        {
            self.request_exit(ExitReason::FramesDone);
        }
        if self.exit.is_some() {
            return;
        }
        self.ack_due_configure();
        self.apply_due_inputs();
        self.sweep_input_ids();
        self.again = false;
        let now = self.clock.now();
        if self.pacer.poll(now) != Decision::Render || !self.render.pool.can_acquire() {
            return;
        }
        match self.render_frame() {
            Ok(true) => {
                let now = self.clock.now();
                self.again =
                    self.pacer.poll(now) == Decision::Render && self.render.pool.can_acquire();
            }
            Ok(false) => {}
            Err(e) => self.request_exit(ExitReason::Failed(e.to_string())),
        }
    }

    /// Returns the next time at which work becomes due without an event. A due frame that
    /// waits for a buffer release adds no deadline, because the release is an event.
    pub fn deadline(&self) -> Option<Timestamp> {
        let now = self.clock.now();
        let sweep = (!self.marking.matcher.is_empty()).then(|| now + SWEEP_INTERVAL);
        [
            self.exit_at,
            self.drain_until,
            self.window.queue.next_due(),
            self.inputs.next_due(),
            self.pacer
                .deadline()
                .filter(|t| *t > now || self.render.pool.can_acquire()),
            sweep,
        ]
        .into_iter()
        .flatten()
        .min()
    }
}
