//! When frame callbacks reach clients.

use fakeland_types::Nanos;
use fakeland_types::dur::{Dur, as_dur};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// When the scheduler sends frame callbacks.
///
/// A callback of a content update fires at the first dispatch at or after the vblank that presents
/// or discards the update. A callback of an update without content fires at the first dispatch
/// after the update applies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CallbackPolicy {
    /// Dispatch right after each vblank.
    #[default]
    AtVblank,
    /// Dispatch at each vblank minus a lead. The lead gives clients a render window before the
    /// next latch. VRR, manual and unthrottled outputs dispatch at each frame.
    BeforeVblank {
        /// The time between the dispatch and the vblank.
        #[serde(with = "as_dur")]
        #[schemars(with = "Dur")]
        lead: Nanos,
    },
    /// Dispatch as soon as a callback becomes due, without waiting for a vblank.
    OnPresent,
}
