//! fakeland_marks_v1 for the test compositor. Surface marks are double-buffered and recorded
//! on the next commit of their surface.

use fakeland_protocol::server::fakeland_marks_manager_v1::{self, FakelandMarksManagerV1};
use fakeland_protocol::server::fakeland_surface_marks_v1::{self, FakelandSurfaceMarksV1};
use fakeland_protocol::wire::{WireSpan, WireTime, join_u64};
use wayland_server::protocol::wl_surface::WlSurface;
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource};

use super::state::{Span, State};

impl GlobalDispatch<FakelandMarksManagerV1, ()> for State {
    fn bind(
        state: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<FakelandMarksManagerV1>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        state.managers.push(data_init.init(resource, ()));
    }
}

impl Dispatch<FakelandMarksManagerV1, ()> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &FakelandMarksManagerV1,
        request: fakeland_marks_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        match request {
            fakeland_marks_manager_v1::Request::GetSurfaceMarks { id, surface } => {
                state.marks.insert(surface.id(), Default::default());
                data_init.init(id, surface);
            }
            fakeland_marks_manager_v1::Request::Mark { name } => state.client_marks.push(name),
            _ => {}
        }
    }
}

impl Dispatch<FakelandSurfaceMarksV1, WlSurface> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &FakelandSurfaceMarksV1,
        request: fakeland_surface_marks_v1::Request,
        surface: &WlSurface,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        use fakeland_surface_marks_v1::Request;
        let Some(pending) = state.marks.get_mut(&surface.id()) else {
            return;
        };
        match request {
            Request::Frame { id_hi, id_lo } => pending.frame = Some(join_u64(id_hi, id_lo)),
            Request::InputHandled { id_hi, id_lo } => pending.handled.push(join_u64(id_hi, id_lo)),
            Request::Span {
                name,
                begin_sec_hi,
                begin_sec_lo,
                begin_nsec,
                end_sec_hi,
                end_sec_lo,
                end_nsec,
            } => {
                let span = WireSpan {
                    begin: WireTime {
                        sec_hi: begin_sec_hi,
                        sec_lo: begin_sec_lo,
                        nsec: begin_nsec,
                    },
                    end: WireTime {
                        sec_hi: end_sec_hi,
                        sec_lo: end_sec_lo,
                        nsec: end_nsec,
                    },
                };
                let (begin, end) = span.decode().expect("valid span");
                pending.spans.push(Span { name, begin, end });
            }
            Request::Mark { name } => pending.marks.push(name),
            _ => {}
        }
    }
}
