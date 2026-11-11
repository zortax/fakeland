//! The frame table: one row per frame, ordered by surface and commit.

use std::io::{self, Write};

use crate::frames::{FrameRecord, FrameTimeline};

use super::field::Row;

/// The column names of the frame table. Timestamps are `CLOCK_MONOTONIC` nanoseconds; spans are
/// milliseconds.
pub const FRAME_COLUMNS: &[&str] = &[
    "surface",
    "commit",
    "window",
    "outcome",
    "callback_sent_ns",
    "committed_ns",
    "ready_ns",
    "latched_ns",
    "latch_output",
    "latch_seq",
    "presented_ns",
    "present_output",
    "present_seq",
    "refresh_ms",
    "present_flags",
    "discarded_ns",
    "released_ns",
    "buffer",
    "buffer_kind",
    "format",
    "modifier",
    "width",
    "height",
    "scale",
    "viewport_w",
    "viewport_h",
    "detached",
    "damage_rects",
    "damage_area",
    "configure_serial",
    "sync",
    "frame_callback",
    "presentation_feedback",
    "fifo_barrier",
    "fifo_wait",
    "target_ns",
    "app_frame_id",
    "inputs",
    "inputs_handled",
    "spans",
    "marks",
    "client_time_ms",
    "gpu_time_ms",
    "queue_time_ms",
    "display_latency_ms",
    "present_interval_ms",
    "vblanks_missed",
];

/// Writes the header row and one row per frame.
pub fn write_frames(timeline: &FrameTimeline, mut w: impl Write) -> io::Result<()> {
    let mut header = Row::default();
    for c in FRAME_COLUMNS {
        header.push(c);
    }
    w.write_all(header.line().as_bytes())?;
    for f in timeline.all_frames() {
        w.write_all(row(timeline, f).line().as_bytes())?;
    }
    w.flush()
}

fn row(tl: &FrameTimeline, f: &FrameRecord) -> Row {
    let mut r = Row::default();
    let b = f.buffer.as_ref();
    let join = |v: Vec<String>| (!v.is_empty()).then(|| v.join(";"));
    r.push(f.surface.0)
        .push(f.commit.0)
        .opt(tl.window_of(f.surface).map(|w| w.0))
        .push(f.outcome().as_str())
        .ts(f.callback_sent)
        .ts(Some(f.committed))
        .ts(f.ready)
        .ts(f.latched.map(|l| l.t))
        .opt(f.latched.map(|l| l.output.0))
        .opt(f.latched.map(|l| l.seq))
        .ts(f.presented.map(|p| p.time))
        .opt(f.presented.map(|p| p.output.0))
        .opt(f.presented.map(|p| p.seq))
        .ms(f.presented.and_then(|p| p.refresh))
        .opt(f.presented.map(|p| p.flags))
        .ts(f.discarded)
        .ts(f.released)
        .opt(b.map(|b| b.id.0))
        .opt(b.map(|b| b.kind.as_str()))
        .opt(b.map(|b| b.fourcc()))
        .opt(b.and_then(|b| b.modifier).map(|m| format!("{m:#x}")))
        .opt(b.map(|b| b.width))
        .opt(b.map(|b| b.height))
        .opt(b.map(|b| b.scale))
        .opt(b.and_then(|b| b.viewport).map(|v| v.w))
        .opt(b.and_then(|b| b.viewport).map(|v| v.h))
        .push(f.detached)
        .push(f.damage.len())
        .push(f.damage_area)
        .opt(f.configure_serial)
        .push(f.sync.as_str())
        .push(f.frame_callback)
        .push(f.presentation_feedback)
        .push(f.fifo_barrier)
        .push(f.fifo_wait)
        .ts(f.target)
        .opt(f.marks.frame_id)
        .push(f.inputs.len())
        .push(f.marks.inputs_handled.len())
        .opt(join(
            f.marks
                .spans
                .iter()
                .map(|s| format!("{}={:.6}", s.name, s.duration().as_millis_f64()))
                .collect(),
        ))
        .opt(join(f.marks.names.clone()))
        .ms(f.client_time)
        .ms(f.gpu_time)
        .ms(f.queue_time)
        .ms(f.display_latency)
        .ms(f.present_interval)
        .opt(f.vblanks_missed);
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::steady;

    #[test]
    fn one_row_per_frame_with_every_column() {
        let tl = FrameTimeline::build(&steady(4).build());
        let mut out = Vec::new();
        write_frames(&tl, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0].split(',').count(), FRAME_COLUMNS.len());
        for l in &lines[1..] {
            assert_eq!(l.split(',').count(), FRAME_COLUMNS.len());
        }
        let cols: Vec<&str> = lines[2].split(',').collect();
        let col = |name: &str| cols[FRAME_COLUMNS.iter().position(|c| *c == name).unwrap()];
        assert_eq!(col("commit"), "2");
        assert_eq!(col("outcome"), "presented");
        assert_eq!(col("gpu_time_ms"), "2.000000");
        assert_eq!(col("format"), "XR24");
        assert_eq!(col("sync"), "explicit");
        assert_eq!(col("vblanks_missed"), "0");
        assert_eq!(col("window"), "1");
    }
}
