//! `fakeland_marks_manager_v1`: the app-mark global. Clients get one marks object per surface and
//! send client-level marks, which land in the trace at receipt.

use fakeland_protocol::server::fakeland_marks_manager_v1::{self, FakelandMarksManagerV1};
use fakeland_protocol::server::fakeland_surface_marks_v1::FakelandSurfaceMarksV1;
use fakeland_protocol::wire::check_name;
use fakeland_trace::{AppMarkKind, Event};
use smithay::reexports::wayland_server::backend::ClientId as WlClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{
    Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource,
};

use crate::marks::with_slot;
use crate::server::{Clients, State};

impl GlobalDispatch<FakelandMarksManagerV1, ()> for State {
    fn bind(
        state: &mut Self,
        _dh: &DisplayHandle,
        client: &Client,
        resource: New<FakelandMarksManagerV1>,
        _data: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        let manager = data_init.init(resource, ());
        if let Some(id) = Clients::id_of(client) {
            state.marks.add(id, manager);
        }
    }
}

impl Dispatch<FakelandMarksManagerV1, ()> for State {
    fn request(
        state: &mut Self,
        _client: &Client,
        manager: &FakelandMarksManagerV1,
        request: fakeland_marks_manager_v1::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        use fakeland_marks_manager_v1::{Error, Request};
        match request {
            Request::GetSurfaceMarks { id, surface } => {
                let taken = with_slot(&surface, |slot| std::mem::replace(&mut slot.claimed, true));
                if taken {
                    manager.post_error(Error::SurfaceExists, "the surface already has marks");
                    return;
                }
                data_init.init::<FakelandSurfaceMarksV1, WlSurface>(id, surface);
            }
            Request::Mark { name } => {
                if check_name(&name).is_err() {
                    manager.post_error(Error::InvalidName, "the mark name is invalid");
                    return;
                }
                state.emit(Event::AppMark {
                    surface: None,
                    commit: None,
                    mark: AppMarkKind::Mark { name },
                });
            }
            Request::Destroy => state.marks.remove(manager),
            _ => {}
        }
    }

    fn destroyed(
        state: &mut Self,
        _client: WlClientId,
        manager: &FakelandMarksManagerV1,
        _data: &(),
    ) {
        state.marks.remove(manager);
    }
}
