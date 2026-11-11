//! The private event queue state. The crate dispatches its own objects on this state, so it
//! never needs the app's dispatch state.

use std::collections::VecDeque;

use fakeland_protocol::client::fakeland_marks_manager_v1::{self, FakelandMarksManagerV1};
use fakeland_protocol::client::fakeland_surface_marks_v1::FakelandSurfaceMarksV1;
use fakeland_protocol::wire::join_u64;
use fakeland_types::InputEventId;
use wayland_client::globals::GlobalListContents;
use wayland_client::protocol::wl_registry::WlRegistry;
use wayland_client::{Connection, Dispatch, QueueHandle};

use crate::input::InputMark;

/// The state of the private event queue: the input ids that arrived and that no call took yet.
#[derive(Debug, Default)]
pub(crate) struct QueueState {
    /// The received input ids, oldest first.
    pub(crate) inputs: VecDeque<InputMark>,
}

impl Dispatch<FakelandMarksManagerV1, ()> for QueueState {
    fn event(
        state: &mut Self,
        _: &FakelandMarksManagerV1,
        event: fakeland_marks_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let fakeland_marks_manager_v1::Event::InputEvent { id_hi, id_lo, kind } = event {
            state.inputs.push_back(InputMark {
                id: InputEventId(join_u64(id_hi, id_lo)),
                kind: kind.into_result().ok(),
            });
        }
    }
}

impl Dispatch<FakelandSurfaceMarksV1, ()> for QueueState {
    fn event(
        _: &mut Self,
        _: &FakelandSurfaceMarksV1,
        _: <FakelandSurfaceMarksV1 as wayland_client::Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WlRegistry, GlobalListContents> for QueueState {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: <WlRegistry as wayland_client::Proxy>::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
