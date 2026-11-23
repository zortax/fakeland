//! Lock timers: an unlock or a second lock a delay after the first `locked`. A timer thread
//! sends `wl_display.sync`, and the action runs on the event queue at its `done`.

use std::time::Duration;

use serde_json::json;
use wayland_client::protocol::wl_callback::{self, WlCallback};
use wayland_client::{Connection, Dispatch, QueueHandle};

use crate::app::App;
use crate::log;

/// The action of a lock timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Timer {
    /// Unlock the session and end the client.
    Unlock,
    /// Lock the session again and keep the first lock object.
    LockAgain,
}

impl App {
    /// Starts the lock timers of the command line once.
    pub fn start_lock_timers(&mut self, conn: &Connection, qh: &QueueHandle<Self>) {
        if self.timers_started {
            return;
        }
        self.timers_started = true;
        let timers = [
            (self.args.unlock_after_ms, Timer::Unlock),
            (self.args.lock_again_after_ms, Timer::LockAgain),
        ];
        for (ms, timer) in timers {
            let Some(ms) = ms else {
                continue;
            };
            let (conn, qh) = (conn.clone(), qh.clone());
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(ms));
                conn.display().sync(&qh, timer);
                if let Err(e) = conn.flush() {
                    eprintln!("fakeland-layerclient: {e}");
                }
            });
        }
    }
}

impl Dispatch<WlCallback, Timer> for App {
    fn event(
        app: &mut Self,
        _: &WlCallback,
        event: wl_callback::Event,
        timer: &Timer,
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if !matches!(event, wl_callback::Event::Done { .. }) {
            return;
        }
        match timer {
            Timer::Unlock => app.unlock(),
            Timer::LockAgain => {
                log::event("lock_again", json!({}));
                app.create_lock(qh);
            }
        }
    }
}
