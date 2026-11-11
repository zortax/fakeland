//! Derived stage times of each frame. Present intervals and missed vblanks count between
//! presented frames, the refreshes that show new content.

use crate::event::SyncKind;
use crate::frames::FrameTimeline;

/// Fills the derived stage times of every frame.
pub(crate) fn fill(tl: &mut FrameTimeline) {
    for frames in tl.frames.values_mut() {
        for f in frames.iter_mut() {
            f.client_time = f.callback_sent.map(|cb| f.committed - cb);
            f.gpu_time = match (f.sync, f.ready) {
                (SyncKind::None, _) | (_, None) => None,
                (_, Some(ready)) => Some(ready - f.committed),
            };
            f.queue_time = f.latched.map(|l| l.t - f.effective_ready());
            f.display_latency = f.presented.map(|p| p.time - f.committed);
        }
        let mut order: Vec<usize> = (0..frames.len())
            .filter(|i| frames[*i].is_presented_frame())
            .collect();
        order.sort_by_key(|i| {
            let p = frames[*i].presented.map(|p| (p.time, p.seq));
            (p, *i)
        });
        for pair in order.windows(2) {
            let (Some(prev), Some(cur)) = (frames[pair[0]].presented, frames[pair[1]].presented)
            else {
                continue;
            };
            let f = &mut frames[pair[1]];
            f.present_interval = Some(cur.time - prev.time);
            f.vblanks_missed =
                (prev.output == cur.output).then(|| cur.seq.saturating_sub(prev.seq + 1));
        }
    }
}
