//! The per-frame load plan: the minimum render time of a frame, with periodic jank, and the CPU
//! burn per frame.

use fakeland_types::Nanos;
use serde::Serialize;

/// How the client waits out a render delay.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum DelayKind {
    /// Busy-wait on the clock. The wait ends within a few microseconds of the deadline.
    #[default]
    Spin,
    /// Sleep in the kernel. The wait ends at the deadline plus the scheduler latency.
    Sleep,
}

/// The injected load of each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct LoadPlan {
    /// The minimum time from render begin to commit.
    pub render_delay: Nanos,
    /// Every frame whose id is a multiple of this number gets the jank on top. Zero disables
    /// jank.
    pub jank_every: u64,
    /// The extra minimum render time of a janky frame.
    pub jank: Nanos,
    /// CPU work per frame, done after drawing.
    pub cpu_burn: Nanos,
    /// How the render delay is waited out.
    pub kind: DelayKind,
}

impl LoadPlan {
    /// Returns true when the frame gets jank. Frame ids start at 1.
    pub fn is_janky(&self, frame_id: u64) -> bool {
        self.jank_every > 0 && self.jank.0 > 0 && frame_id.is_multiple_of(self.jank_every)
    }

    /// Returns the minimum time from render begin to commit for a frame.
    pub fn min_render_time(&self, frame_id: u64) -> Nanos {
        if self.is_janky(frame_id) {
            self.render_delay + self.jank
        } else {
            self.render_delay
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_nth_frame_is_janky() {
        let plan = LoadPlan {
            render_delay: Nanos::from_millis(7),
            jank_every: 10,
            jank: Nanos::from_millis(25),
            ..LoadPlan::default()
        };
        assert_eq!(plan.min_render_time(1), Nanos::from_millis(7));
        assert_eq!(plan.min_render_time(9), Nanos::from_millis(7));
        assert_eq!(plan.min_render_time(10), Nanos::from_millis(32));
        assert_eq!(plan.min_render_time(20), Nanos::from_millis(32));
        assert!(!plan.is_janky(11));
    }

    #[test]
    fn zero_disables_jank() {
        let plan = LoadPlan {
            jank_every: 0,
            jank: Nanos::from_millis(5),
            ..LoadPlan::default()
        };
        assert!(!plan.is_janky(10));
        let plan = LoadPlan {
            jank_every: 3,
            ..LoadPlan::default()
        };
        assert!(!plan.is_janky(3));
    }
}
