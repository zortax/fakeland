//! A minimal fakeland_marks_v1 implementation for the test compositor. It checks requests with
//! the shared wire helpers and raises the protocol errors.

use fakeland_protocol::server::fakeland_marks_manager_v1::{self, FakelandMarksManagerV1};
use fakeland_protocol::server::fakeland_surface_marks_v1::{self, FakelandSurfaceMarksV1};
use fakeland_protocol::wire::{WireSpan, WireTime, check_name, join_u64};
use fakeland_types::InputEventId;
use wayland_server::protocol::wl_surface::WlSurface;
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource};

use super::record::{Record, SpanRecord};
use super::state::ServerState;

impl GlobalDispatch<FakelandMarksManagerV1, ()> for ServerState {
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

impl Dispatch<FakelandMarksManagerV1, ()> for ServerState {
    fn request(
        state: &mut Self,
        _: &Client,
        manager: &FakelandMarksManagerV1,
        request: fakeland_marks_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        use fakeland_marks_manager_v1::{Error, Request};
        match request {
            Request::GetSurfaceMarks { id, surface } => {
                if state.pending.contains_key(&surface.id()) {
                    manager.post_error(Error::SurfaceExists, "the surface has marks");
                    return;
                }
                state.pending.insert(surface.id(), Default::default());
                data_init.init(id, surface);
            }
            Request::Mark { name } => {
                if check_name(&name).is_err() {
                    manager.post_error(Error::InvalidName, "invalid name");
                    return;
                }
                state.record(Record::ClientMark(name));
            }
            // Destroy leaves the surface marks objects alive.
            _ => {}
        }
    }
}

impl Dispatch<FakelandSurfaceMarksV1, WlSurface> for ServerState {
    fn request(
        state: &mut Self,
        _: &Client,
        marks: &FakelandSurfaceMarksV1,
        request: fakeland_surface_marks_v1::Request,
        surface: &WlSurface,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        use fakeland_surface_marks_v1::{Error, Request};
        if matches!(request, Request::Destroy) {
            state.pending.remove(&surface.id());
            return;
        }
        let Some(pending) = state
            .pending
            .get_mut(&surface.id())
            .filter(|_| surface.is_alive())
        else {
            marks.post_error(Error::NoSurface, "the surface is destroyed");
            return;
        };
        match request {
            Request::Frame { id_hi, id_lo } => pending.frame = Some(join_u64(id_hi, id_lo)),
            Request::InputHandled { id_hi, id_lo } => {
                let id = InputEventId(join_u64(id_hi, id_lo));
                if !pending.inputs.contains(&id) {
                    pending.inputs.push(id);
                }
            }
            Request::Span {
                name,
                begin_sec_hi,
                begin_sec_lo,
                begin_nsec,
                end_sec_hi,
                end_sec_lo,
                end_nsec,
            } => {
                if check_name(&name).is_err() {
                    marks.post_error(Error::InvalidName, "invalid name");
                    return;
                }
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
                match span.decode() {
                    Ok((begin, end)) => pending.spans.push(SpanRecord { name, begin, end }),
                    Err(_) => marks.post_error(Error::InvalidSpan, "invalid span"),
                }
            }
            Request::Mark { name } => {
                if check_name(&name).is_err() {
                    marks.post_error(Error::InvalidName, "invalid name");
                    return;
                }
                pending.marks.push(name);
            }
            _ => {}
        }
    }
}
