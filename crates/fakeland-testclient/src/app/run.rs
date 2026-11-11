//! The event loop. Each pass dispatches the read events, runs the due work, flushes, and then
//! polls the connection and the deadline timer. The wake time after the poll is the receive time
//! of every event of the following read.

use std::io::ErrorKind;

use fakeland_types::Clock;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::io::Errno;
use rustix::time::Timespec;
use wayland_client::backend::WaylandError;
use wayland_client::{DispatchError, EventQueue};

use super::{App, ExitReason, Timer};

/// Runs the client until it exits and returns why.
pub fn run(app: &mut App, queue: &mut EventQueue<App>) -> ExitReason {
    let mut timer = match Timer::new() {
        Ok(t) => t,
        Err(e) => return ExitReason::Failed(format!("timerfd: {e}")),
    };
    loop {
        if let Err(e) = queue.dispatch_pending(app) {
            return dispatch_error(e);
        }
        app.tick();
        if let Some(reason) = app.exit.take() {
            return reason;
        }
        let blocked = match app.conn.flush() {
            Ok(()) => false,
            Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => true,
            Err(e) => return wayland_error(e),
        };
        let Some(guard) = queue.prepare_read() else {
            continue;
        };
        if let Err(e) = timer.arm(app.deadline()) {
            return ExitReason::Failed(format!("timerfd: {e}"));
        }
        let (readable, fired) = {
            let conn_fd = guard.connection_fd();
            let timer_fd = timer.fd();
            let mut events = PollFlags::IN;
            if blocked {
                events |= PollFlags::OUT;
            }
            let mut fds = [
                PollFd::new(&conn_fd, events),
                PollFd::new(&timer_fd, PollFlags::IN),
            ];
            let zero = Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            };
            match poll(&mut fds, app.again.then_some(&zero)) {
                Ok(_) => {}
                Err(Errno::INTR) => continue,
                Err(e) => return ExitReason::Failed(format!("poll: {e}")),
            }
            let input = PollFlags::IN | PollFlags::ERR | PollFlags::HUP;
            (
                fds[0].revents().intersects(input),
                fds[1].revents().contains(PollFlags::IN),
            )
        };
        app.wake = app.clock.now();
        if readable {
            match guard.read() {
                Ok(_) => {}
                Err(WaylandError::Io(e)) if e.kind() == ErrorKind::WouldBlock => {}
                Err(e) => return wayland_error(e),
            }
        } else {
            drop(guard);
        }
        if fired {
            timer.clear();
        }
    }
}

fn dispatch_error(e: DispatchError) -> ExitReason {
    match e {
        DispatchError::Backend(e) => wayland_error(e),
        DispatchError::BadMessage { .. } => ExitReason::Protocol(e.to_string()),
    }
}

fn wayland_error(e: WaylandError) -> ExitReason {
    match e {
        WaylandError::Protocol(p) => ExitReason::Protocol(p.to_string()),
        WaylandError::Io(e) => ExitReason::Disconnected(e.to_string()),
    }
}
