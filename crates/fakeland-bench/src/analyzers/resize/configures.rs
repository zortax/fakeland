//! The configures of a toplevel with their acks and answering commits.

use fakeland_trace::{Event, FrameRecord, Trace};
use fakeland_types::{CommitId, SurfaceId, Timestamp};
use serde::{Deserialize, Serialize};

use crate::analyzers::util::logical_size;

/// One configure and how the client answered it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConfigureRecord {
    /// The serial.
    pub serial: u32,
    /// The configured logical width; 0 lets the client choose.
    pub width: i32,
    /// The configured logical height; 0 lets the client choose.
    pub height: i32,
    /// The send time.
    pub sent: Timestamp,
    /// The ack time of this serial.
    pub acked: Option<Timestamp>,
    /// The client acked a newer serial and never this one.
    pub superseded: bool,
    /// The first commit with a buffer that carries this serial or a newer one.
    pub commit: Option<CommitId>,
    /// The commit time of that commit.
    pub committed: Option<Timestamp>,
    /// The modeled presentation of that commit.
    pub presented: Option<Timestamp>,
    /// The logical buffer size of that commit.
    pub buffer_size: Option<(f64, f64)>,
}

impl ConfigureRecord {
    /// Returns true when the configure sets a size.
    pub fn sized(&self) -> bool {
        self.width > 0 && self.height > 0
    }
}

/// Returns the configures of a surface in send order with their answers.
pub fn configures_of(
    trace: &Trace,
    surface: SurfaceId,
    frames: &[FrameRecord],
) -> Vec<ConfigureRecord> {
    let mut out: Vec<ConfigureRecord> = Vec::new();
    let mut acks: Vec<(u32, Timestamp)> = Vec::new();
    for r in &trace.records {
        match &r.event {
            Event::Configure {
                surface: s,
                serial,
                width,
                height,
                ..
            } if *s == surface => out.push(ConfigureRecord {
                serial: *serial,
                width: *width,
                height: *height,
                sent: r.t,
                acked: None,
                superseded: false,
                commit: None,
                committed: None,
                presented: None,
                buffer_size: None,
            }),
            Event::AckConfigure { surface: s, serial } if *s == surface => {
                acks.push((*serial, r.t))
            }
            _ => {}
        }
    }
    let max_ack = acks.iter().map(|a| a.0).max();
    for c in &mut out {
        c.acked = acks.iter().find(|a| a.0 == c.serial).map(|a| a.1);
        c.superseded = c.acked.is_none() && max_ack.is_some_and(|m| m > c.serial);
        let answer = frames.iter().find(|f| {
            f.buffer.is_some()
                && f.committed >= c.sent
                && f.configure_serial.is_some_and(|s| s >= c.serial)
        });
        if let Some(f) = answer {
            c.commit = Some(f.commit);
            c.committed = Some(f.committed);
            c.presented = f.presented_time();
            c.buffer_size = f.buffer.as_ref().map(logical_size);
        }
    }
    out
}
