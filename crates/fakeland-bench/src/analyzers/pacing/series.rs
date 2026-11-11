//! Frames per second over whole seconds since the first presentation.

use fakeland_types::Timestamp;

use super::FpsBucket;

/// Counts frames per whole second. Frames of a partial last second are left out.
pub fn fps_series(times: &[Timestamp]) -> Vec<FpsBucket> {
    let (Some(first), Some(last)) = (times.first(), times.last()) else {
        return Vec::new();
    };
    let whole = (last.since(*first).as_nanos() / 1_000_000_000) as u32;
    let mut buckets: Vec<FpsBucket> = (0..whole)
        .map(|second| FpsBucket { second, frames: 0 })
        .collect();
    for t in times {
        let s = (t.since(*first).as_nanos() / 1_000_000_000) as usize;
        if let Some(b) = buckets.get_mut(s) {
            b.frames += 1;
        }
    }
    buckets
}
