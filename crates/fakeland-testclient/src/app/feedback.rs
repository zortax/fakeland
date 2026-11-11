//! Compositor feedback on committed frames: frame callbacks, presentation feedback, buffer
//! releases and the presentation clock.

use fakeland_types::{Clock, Nanos};

use super::App;
use crate::log::LogEvent;
use crate::wl::{BufferTag, Presentation};

impl App {
    /// Handles wl_callback.done of the callback that a frame's commit requested.
    pub fn frame_done(&mut self, frame_id: u64, callback_ms: u32) {
        let now = self.clock.now();
        self.render.anim.callback_ms = Some(callback_ms);
        self.pacer.callback_done();
        self.log.write(
            now,
            &LogEvent::FrameDone {
                frame_id,
                callback_ms,
            },
        );
    }

    /// Handles wp_presentation_feedback.presented.
    pub fn presented(&mut self, frame_id: u64, p: Presentation) {
        let now = self.clock.now();
        self.render.anim.presented = Some(p.time);
        self.render.anim.refresh = (p.refresh > 0).then_some(Nanos(p.refresh as i64));
        self.feedback_arrived();
        self.log.write(
            now,
            &LogEvent::Presented {
                frame_id,
                time: p.time.0,
                refresh: p.refresh,
                seq: p.seq,
                flags: p.flags,
            },
        );
    }

    /// Handles wp_presentation_feedback.discarded.
    pub fn discarded(&mut self, frame_id: u64) {
        let now = self.clock.now();
        self.feedback_arrived();
        self.log.write(now, &LogEvent::Discarded { frame_id });
    }

    /// Handles wl_buffer.release.
    pub fn buffer_released(&mut self, tag: BufferTag) {
        let now = self.clock.now();
        if let Some(frame_id) = self.render.pool.release(tag) {
            self.log.write(
                now,
                &LogEvent::Release {
                    buffer: tag.0,
                    frame_id,
                },
            );
        }
    }

    /// Handles wp_presentation.clock_id.
    pub fn presentation_clock(&mut self, clock_id: u32) {
        let now = self.clock.now();
        self.log
            .write(now, &LogEvent::PresentationClock { clock_id });
    }

    fn feedback_arrived(&mut self) {
        self.render.feedback_pending = self.render.feedback_pending.saturating_sub(1);
    }
}
