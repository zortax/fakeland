//! Rendering and committing one frame: draw the scenes into the scratch frame, compute damage,
//! upload into a free buffer, inject load, then attach, damage, request callbacks and feedback,
//! apply fifo and commit-timing state, mark the frame and commit. The log lines of a frame are
//! written after the commit and flush, so logging adds no time between render end and commit.

use super::{AnimClock, App, Previous, frame_damage};
use crate::draw::{BYTES_PER_PIXEL, Canvas};
use crate::load::{burn, wait_until};
use crate::log::{CommitInfo, LogEvent};
use crate::scene::DrawContext;
use crate::wl::{FeedbackTag, FrameTag, PoolError, Scale, timer_args};
use fakeland_types::{Clock, Logical, Nanos, Size, Timestamp};

impl App {
    /// Renders and commits a frame when a buffer is free. Returns false when every buffer is
    /// busy.
    pub fn render_frame(&mut self) -> Result<bool, PoolError> {
        let logical = self.logical_size();
        let scale = self.scale.current();
        let size = scale.buffer_size(logical);
        let Some(slot) = self.render.pool.acquire(size, &self.qh)? else {
            return Ok(false);
        };
        let frame_id = self.render.frame_id + 1;
        let begin = self.clock.now();

        let len = size.w as usize * size.h as usize * BYTES_PER_PIXEL;
        self.render.scratch.resize(len, 0);
        let time = self.anim_time(begin);
        let mut canvas = Canvas::new(&mut self.render.scratch, size, scale.factor());
        let state = self.scenes.draw(
            &mut canvas,
            &DrawContext {
                window: logical,
                frame_id,
                time,
            },
        );
        let previous = self.render.last_size.map(|size| Previous {
            pixels: &self.render.last,
            size,
        });
        let damage = frame_damage(self.args.damage, previous, &self.render.scratch, size);
        self.render.pool.upload(slot, &self.render.scratch)?;
        burn(&self.clock, self.load.cpu_burn);
        let janky = self.load.is_janky(frame_id);
        wait_until(
            &self.clock,
            begin + self.load.min_render_time(frame_id),
            self.load.kind,
        );
        let end = self.clock.now();

        self.apply_surface_scale(scale, logical);
        let surface = &self.surface.surface;
        let (buffer, buffer_number) = self.render.pool.buffer(slot);
        surface.attach(Some(buffer), 0, 0);
        if let Some(d) = damage {
            if self.surface.has_buffer_damage() {
                surface.damage_buffer(d.x, d.y, d.w, d.h);
            } else {
                let l = d.to_logical(scale.factor());
                surface.damage(l.x, l.y, l.w, l.h);
            }
        }
        let callback = self.pacer.throttled() || self.args.request_callbacks;
        if callback {
            surface.frame(&self.qh, FrameTag(frame_id));
        }
        let feedback = match (&self.globals.presentation, self.args.no_feedback) {
            (Some(p), false) => {
                p.feedback(surface, &self.qh, FeedbackTag(frame_id));
                self.render.feedback_pending += 1;
                true
            }
            _ => false,
        };
        if let Some(fifo) = &self.surface.fifo {
            fifo.wait_barrier();
            fifo.set_barrier();
        }
        let target = match (&self.surface.timer, self.args.target_delay) {
            (Some(timer), Some(delay)) => {
                let t = self.clock.now() + delay;
                let (hi, lo, ns) = timer_args(t);
                timer.set_timestamp(hi, lo, ns);
                Some(t)
            }
            _ => None,
        };
        let handled = std::mem::take(&mut self.marking.handled);
        if let Some(marks) = &self.marking.surface {
            marks.frame(frame_id);
            for id in &handled {
                marks.input_handled(*id);
            }
            // Begin precedes end and the name is a constant, so the span is valid.
            let _ = marks.span("render", begin, end);
            if frame_id == 1 {
                let _ = marks.mark("first_frame");
            }
        }
        surface.commit();
        let flushed = self.conn.flush();
        let committed = self.clock.now();

        self.render.pool.attach(slot, frame_id);
        self.render.frame_id = frame_id;
        std::mem::swap(&mut self.render.scratch, &mut self.render.last);
        self.render.last_size = Some(size);
        let skipped = self.pacer.rendered(committed, callback);

        self.log.write(begin, &LogEvent::RenderBegin { frame_id });
        self.log
            .write(end, &LogEvent::RenderEnd { frame_id, janky });
        let info = CommitInfo {
            frame_id,
            buffer: buffer_number,
            size: [logical.w, logical.h],
            buffer_size: [size.w, size.h],
            scale: scale.factor(),
            damage: damage.map(|d| [d.x, d.y, d.w, d.h]),
            handled: handled.iter().map(|id| id.0).collect(),
            callback,
            feedback,
            fifo: self.surface.fifo.is_some(),
            target: target.map(|t| t.0),
            render_begin: begin.0,
            render_end: end.0,
            state,
        };
        self.log.write(committed, &LogEvent::Commit(Box::new(info)));
        if skipped > 0 {
            self.log.write(
                committed,
                &LogEvent::Skipped {
                    reason: "ticks".to_owned(),
                    count: skipped,
                },
            );
        }
        if frame_id == 1 {
            self.phase("first_frame");
        }
        if let Err(e) = flushed {
            self.warn(format!("flush after commit: {e}"));
        }
        self.check_frame_limit(committed);
        Ok(true)
    }

    fn anim_time(&self, begin: Timestamp) -> Timestamp {
        match self.args.anim_clock {
            AnimClock::Render => begin,
            clock => clock.time(begin, &self.render.anim),
        }
    }

    fn apply_surface_scale(&mut self, scale: Scale, logical: Size<Logical>) {
        if self.render.applied == Some((scale, logical)) {
            return;
        }
        if let Some(viewport) = &self.surface.viewport {
            viewport.set_destination(logical.w, logical.h);
        }
        if self.surface.has_buffer_scale() {
            self.surface.surface.set_buffer_scale(scale.buffer_scale());
        }
        self.render.applied = Some((scale, logical));
    }

    fn check_frame_limit(&mut self, now: Timestamp) {
        let Some(limit) = self.args.exit_after_frames else {
            return;
        };
        if self.render.frame_id >= limit && self.drain_until.is_none() {
            self.pacer.stop();
            self.drain_until = Some(now + Nanos::from_secs(1));
        }
    }
}
