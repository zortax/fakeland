//! The loop timer: a `CLOCK_MONOTONIC` timerfd armed with an absolute deadline. A timerfd
//! applies no timer slack, so the loop wakes within microseconds of a deadline.

use std::io;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use fakeland_types::Timestamp;
use rustix::time::{
    Itimerspec, TimerfdClockId, TimerfdFlags, TimerfdTimerFlags, Timespec, timerfd_create,
    timerfd_settime,
};

/// A one-shot absolute timer.
#[derive(Debug)]
pub struct Timer {
    fd: OwnedFd,
    armed: Option<Timestamp>,
}

impl Timer {
    /// Creates a disarmed timer.
    pub fn new() -> io::Result<Self> {
        let fd = timerfd_create(
            TimerfdClockId::Monotonic,
            TimerfdFlags::CLOEXEC | TimerfdFlags::NONBLOCK,
        )?;
        Ok(Self { fd, armed: None })
    }

    /// Arms the timer for a deadline, or disarms it for `None`.
    pub fn arm(&mut self, deadline: Option<Timestamp>) -> io::Result<()> {
        if self.armed == deadline {
            return Ok(());
        }
        let value = match deadline {
            // A zero value disarms a timerfd, so the earliest deadline is 1 ns.
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
        self.armed = deadline;
        Ok(())
    }

    /// Reads the expiration count after the timer fired.
    pub fn clear(&mut self) {
        let mut buf = [0u8; 8];
        // A read error means the timer did not fire, which needs no handling.
        let _ = rustix::io::read(&self.fd, &mut buf);
        self.armed = None;
    }

    /// Returns the timer fd for polling.
    pub fn fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::{Clock, MonotonicClock, Nanos};
    use rustix::event::{PollFd, PollFlags, poll};

    use super::*;

    #[test]
    fn fires_at_the_deadline() {
        let clock = MonotonicClock;
        let mut t = Timer::new().unwrap();
        let deadline = clock.now() + Nanos::from_millis(3);
        t.arm(Some(deadline)).unwrap();
        let fd = t.fd();
        let mut fds = [PollFd::new(&fd, PollFlags::IN)];
        poll(&mut fds, None).unwrap();
        assert!(clock.now() >= deadline);
        t.clear();
        t.arm(None).unwrap();
    }

    #[test]
    fn past_deadline_fires_at_once() {
        let mut t = Timer::new().unwrap();
        t.arm(Some(Timestamp(0))).unwrap();
        let fd = t.fd();
        let mut fds = [PollFd::new(&fd, PollFlags::IN)];
        let zero = Timespec {
            tv_sec: 1,
            tv_nsec: 0,
        };
        assert_eq!(poll(&mut fds, Some(&zero)).unwrap(), 1);
    }
}
