//! A `CLOCK_MONOTONIC` timerfd armed with absolute deadlines. The event loop polls one end of the
//! timer and the state arms the other end.

use std::io;
use std::os::fd::OwnedFd;

use fakeland_types::Timestamp;
use smithay::reexports::rustix::io::{Errno, read};
use smithay::reexports::rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};

/// The arming handle of the deadline timer.
#[derive(Debug)]
pub struct DeadlineTimer {
    fd: OwnedFd,
    armed: Option<Timestamp>,
}

impl DeadlineTimer {
    /// Creates the timer. The returned fd is the end the event loop polls.
    pub fn new() -> io::Result<(Self, OwnedFd)> {
        let fd = timerfd_create(
            TimerfdClockId::Monotonic,
            TimerfdFlags::NONBLOCK | TimerfdFlags::CLOEXEC,
        )?;
        let poll = fd.try_clone()?;
        Ok((Self { fd, armed: None }, poll))
    }

    /// Arms the timer at an absolute time, or disarms it with `None`. An unchanged deadline
    /// costs no syscall.
    pub fn arm(&mut self, at: Option<Timestamp>) -> io::Result<()> {
        if at == self.armed {
            return Ok(());
        }
        let value = match at {
            Some(t) => {
                let (secs, nanos) = t.max(Timestamp(1)).to_secs_nanos();
                Timespec {
                    tv_sec: secs as i64,
                    tv_nsec: nanos as i64,
                }
            }
            None => Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
        };
        let spec = Itimerspec {
            it_interval: Timespec {
                tv_sec: 0,
                tv_nsec: 0,
            },
            it_value: value,
        };
        timerfd_settime(&self.fd, TimerfdTimerFlags::ABSTIME, &spec)?;
        self.armed = at;
        Ok(())
    }

    /// Marks the armed deadline as fired, so the next `arm` call sets the timer again.
    pub fn fired(&mut self) {
        self.armed = None;
    }

    /// Consumes the expiration count of the polled end.
    pub fn drain(poll: &OwnedFd) {
        let mut buf = [0u8; 8];
        match read(poll, &mut buf) {
            Ok(_) | Err(Errno::AGAIN) => {}
            Err(e) => tracing::warn!("timerfd read failed: {e}"),
        }
    }
}
