//! Window management: configures are logged when they arrive, acked after the configure delay,
//! and answered with an urgent frame. The first ack starts the pacer after the first-frame
//! delay. A scale hint change also asks for an urgent frame.

use fakeland_types::Clock;

use super::{App, ExitReason, ToplevelConfigure};
use crate::log::LogEvent;

impl App {
    /// Records xdg_toplevel.configure until its xdg_surface.configure arrives.
    pub fn toplevel_configure(&mut self, width: i32, height: i32, states: Vec<String>) {
        self.window.toplevel = Some(ToplevelConfigure {
            width,
            height,
            states,
        });
    }

    /// Records xdg_surface.configure and queues its ack.
    pub fn surface_configure(&mut self, serial: u32) {
        let now = self.clock.now();
        let pending = self.pending_configure(serial);
        self.log.write(
            now,
            &LogEvent::Configure {
                serial,
                width: pending.width,
                height: pending.height,
                states: pending.states.clone(),
            },
        );
        self.window
            .queue
            .push(now + self.args.resize_delay, pending);
    }

    /// Acks the newest due configure.
    pub fn ack_due_configure(&mut self) {
        let now = self.clock.now();
        let Some((cfg, superseded)) = self.window.queue.take_due(now) else {
            return;
        };
        let first = self.window.size.is_none();
        let size = cfg.size(self.window.size, self.args.size);
        self.surface.xdg_surface.ack_configure(cfg.serial);
        self.window.size = Some(size);
        let now = self.clock.now();
        self.log.write(
            now,
            &LogEvent::Ack {
                serial: cfg.serial,
                width: size.w,
                height: size.h,
                superseded,
            },
        );
        if first {
            self.phase("configured");
            self.log_scale();
            self.pacer.start(now + self.args.first_frame_delay);
        } else {
            self.pacer.mark_urgent();
        }
    }

    /// Handles xdg_toplevel.close.
    pub fn closed(&mut self) {
        let now = self.clock.now();
        self.log.write(now, &LogEvent::Close);
        self.request_exit(ExitReason::Closed);
    }

    /// Destroys the surface objects and flushes, so the window unmaps before the client exits.
    pub fn shutdown(&mut self) {
        self.surface.destroy();
        // The process exits next, so a failed flush changes nothing.
        let _ = self.conn.flush();
    }

    /// Reacts to a changed scale hint.
    pub fn scale_hint_changed(&mut self) {
        let current = self.scale.current();
        if self.render.applied.is_some_and(|(s, _)| s == current) {
            return;
        }
        if self.window.size.is_some() {
            self.log_scale();
            self.pacer.mark_urgent();
        }
    }

    fn log_scale(&mut self) {
        let s = self.scale.current();
        if self.window.logged_scale.replace(s) == Some(s) {
            return;
        }
        let now = self.clock.now();
        self.log.write(
            now,
            &LogEvent::Scale {
                kind: s.kind().to_owned(),
                scale120: s.scale120(),
                buffer_scale: s.buffer_scale(),
            },
        );
    }
}
