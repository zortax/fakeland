//! wp_commit_timing_manager_v1 and wp_commit_timer_v1, which have no events, and the encoding
//! of a target time for wp_commit_timer_v1.set_timestamp.

use fakeland_types::Timestamp;
use wayland_protocols::wp::commit_timing::v1::client::{
    wp_commit_timer_v1::WpCommitTimerV1, wp_commit_timing_manager_v1::WpCommitTimingManagerV1,
};

use crate::app::App;

wayland_client::delegate_noop!(App: WpCommitTimingManagerV1);
wayland_client::delegate_noop!(App: WpCommitTimerV1);

/// Splits a timestamp into the `tv_sec_hi`, `tv_sec_lo` and `tv_nsec` arguments.
pub fn timer_args(t: Timestamp) -> (u32, u32, u32) {
    let (secs, nanos) = t.to_secs_nanos();
    ((secs >> 32) as u32, secs as u32, nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_seconds() {
        let t = Timestamp::from_secs_nanos((1u64 << 32) | 9, 123);
        assert_eq!(timer_args(t), (1, 9, 123));
    }
}
