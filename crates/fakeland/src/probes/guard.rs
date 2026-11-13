//! The readback budget of a probe. A readback runs on the compositor thread, so the probe
//! starts it clear of output wakes through [`State::clear_slot`]. The detection keeps the record
//! time of its candidate, so a readback that waits changes no stage time. A readback also waits
//! while the sampler thread holds the GPU gate: a GPU query of the sampler can block the GL calls
//! of the readback for several milliseconds.

use fakeland_types::{Nanos, Timestamp};

use crate::server::State;

/// The smallest span a readback keeps clear of the next output wake.
pub(super) const MIN_GUARD: Nanos = Nanos::from_millis(2);

pub(super) use crate::display::MAX_WAITS as MAX_DEFERRALS;

/// The span after which a readback tries the GPU gate again.
pub(super) const GATE_RETRY: Nanos = Nanos::from_micros(500);

/// How often a readback tries the GPU gate before it runs at once: 100 ms, several times the
/// longest GPU query seen.
pub(super) const MAX_GATE_WAITS: u32 = 200;

/// Returns the span a readback with a known worst cost keeps clear of the next output wake.
pub(super) fn guard_for(worst_cost: Nanos) -> Nanos {
    (worst_cost * 3 / 2).max(MIN_GUARD)
}

/// Returns the time to run a readback, or `None` to run it now.
pub(super) fn readback_slot(state: &State, guard: Nanos) -> Option<Timestamp> {
    state.clear_slot(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guard_covers_the_worst_readback() {
        assert_eq!(guard_for(Nanos::from_millis(1)), MIN_GUARD);
        assert_eq!(guard_for(Nanos::from_millis(4)), Nanos::from_millis(6));
    }
}
