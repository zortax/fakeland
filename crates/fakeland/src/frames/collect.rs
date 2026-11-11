//! Reads the pending state of a commit before smithay caches it: the buffer, damage, frame
//! callbacks, presentation feedback, fifo flags, commit-timing target, acquire point and viewport.
//! Frame callbacks and feedback move out of smithay into the frame pipeline.

use std::time::Duration;

use fakeland_types::{Logical, Physical, Rect, Size, Timestamp};
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_callback::WlCallback;
use smithay::utils::{Monotonic, Time};
use smithay::wayland::commit_timing::CommitTimerStateUserData;
use smithay::wayland::compositor::{BufferAssignment, Damage, SurfaceAttributes, SurfaceData};
use smithay::wayland::drm_syncobj::{DrmSyncPoint, DrmSyncobjCachedState};
use smithay::wayland::fifo::FifoCachedState;
use smithay::wayland::presentation::{
    PresentationFeedbackCachedState, PresentationFeedbackCallback,
};
use smithay::wayland::viewporter::ViewportCachedState;

/// The pending state of one commit.
#[derive(Debug, Default)]
pub struct Collected {
    /// `Some(Some(b))` attaches a buffer, `Some(None)` detaches, `None` keeps the buffer.
    pub buffer: Option<Option<WlBuffer>>,
    /// The buffer scale.
    pub buffer_scale: i32,
    /// Damage in buffer pixels.
    pub damage: Vec<Rect<Physical>>,
    /// The frame callbacks, moved out of smithay.
    pub callbacks: Vec<WlCallback>,
    /// The presentation feedback objects, moved out of smithay.
    pub feedback: Vec<PresentationFeedbackCallback>,
    /// The fifo barrier flags.
    pub fifo: FifoCachedState,
    /// The commit-timing target.
    pub target: Option<Timestamp>,
    /// The explicit-sync acquire point.
    pub acquire: Option<DrmSyncPoint>,
    /// The viewport destination size.
    pub viewport: Option<Size<Logical>>,
}

impl Collected {
    /// Returns true when the commit changes the content: a new buffer or a detach.
    pub fn has_content(&self) -> bool {
        self.buffer.is_some()
    }

    /// Returns the attached buffer.
    pub fn new_buffer(&self) -> Option<&WlBuffer> {
        self.buffer.as_ref().and_then(Option::as_ref)
    }
}

/// Reads and takes the pending state of a surface.
pub fn collect(states: &SurfaceData) -> Collected {
    let mut out = Collected::default();
    {
        let mut guard = states.cached_state.get::<SurfaceAttributes>();
        let attrs = guard.pending();
        out.buffer = attrs.buffer.as_ref().map(|b| match b {
            BufferAssignment::NewBuffer(buffer) => Some(buffer.clone()),
            BufferAssignment::Removed => None,
        });
        out.buffer_scale = attrs.buffer_scale;
        out.damage = attrs
            .damage
            .iter()
            .map(|d| buffer_damage(d, attrs.buffer_scale))
            .collect();
        out.callbacks = std::mem::take(&mut attrs.frame_callbacks);
    }
    out.feedback = std::mem::take(
        &mut states
            .cached_state
            .get::<PresentationFeedbackCachedState>()
            .pending()
            .callbacks,
    );
    out.fifo = *states.cached_state.get::<FifoCachedState>().pending();
    out.target = states
        .data_map
        .get::<CommitTimerStateUserData>()
        .and_then(|s| s.borrow_mut().timestamp.take())
        .map(|ts| {
            let time: Time<Monotonic> = ts.into();
            let d: Duration = time.into();
            Timestamp(d.as_nanos() as u64)
        });
    out.acquire = states
        .cached_state
        .get::<DrmSyncobjCachedState>()
        .pending()
        .acquire_point
        .clone();
    out.viewport = states
        .cached_state
        .get::<ViewportCachedState>()
        .pending()
        .dst
        .map(|s| Size::new(s.w, s.h));
    out
}

/// Converts damage to buffer pixels. Surface damage scales by the buffer scale and saturates.
pub fn buffer_damage(d: &Damage, scale: i32) -> Rect<Physical> {
    match d {
        Damage::Buffer(r) => Rect::new(r.loc.x, r.loc.y, r.size.w, r.size.h),
        Damage::Surface(r) => {
            let s = scale.max(1);
            Rect::new(
                r.loc.x.saturating_mul(s),
                r.loc.y.saturating_mul(s),
                r.size.w.saturating_mul(s),
                r.size.h.saturating_mul(s),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use smithay::utils::Rectangle;

    use super::*;

    #[test]
    fn surface_damage_scales_and_saturates() {
        let d = Damage::Surface(Rectangle::new((10, 20).into(), (30, 40).into()));
        assert_eq!(buffer_damage(&d, 2), Rect::new(20, 40, 60, 80));
        let full = Damage::Surface(Rectangle::new((0, 0).into(), (i32::MAX, i32::MAX).into()));
        assert_eq!(buffer_damage(&full, 2), Rect::new(0, 0, i32::MAX, i32::MAX));
        let b = Damage::Buffer(Rectangle::new((1, 2).into(), (3, 4).into()));
        assert_eq!(buffer_damage(&b, 3), Rect::new(1, 2, 3, 4));
    }
}
