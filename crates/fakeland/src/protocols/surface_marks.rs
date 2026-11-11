//! `fakeland_surface_marks_v1`: double-buffered marks of one surface. Requests fill the pending
//! marks in the surface data, and the next `wl_surface.commit` applies them.

use fakeland_protocol::server::fakeland_surface_marks_v1::{self, FakelandSurfaceMarksV1};
use fakeland_protocol::wire::{WireSpan, WireTime, check_name, join_u64};
use fakeland_types::InputEventId;
use smithay::reexports::wayland_server::backend::ClientId as WlClientId;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::{Client, DataInit, Dispatch, DisplayHandle, Resource};

use crate::marks::{SlotState, with_slot};
use crate::server::State;

impl Dispatch<FakelandSurfaceMarksV1, WlSurface> for State {
    fn request(
        _state: &mut Self,
        _client: &Client,
        marks: &FakelandSurfaceMarksV1,
        request: fakeland_surface_marks_v1::Request,
        surface: &WlSurface,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, Self>,
    ) {
        use fakeland_surface_marks_v1::{Error, Request};
        if matches!(request, Request::Destroy) {
            release(surface);
            return;
        }
        if !surface.is_alive() {
            marks.post_error(Error::NoSurface, "the surface is destroyed");
            return;
        }
        let result = with_slot(surface, |slot| apply(slot, request));
        if let Err((code, message)) = result {
            marks.post_error(code, message);
        }
    }

    fn destroyed(
        _state: &mut Self,
        _client: WlClientId,
        _marks: &FakelandSurfaceMarksV1,
        surface: &WlSurface,
    ) {
        release(surface);
    }
}

type RequestError = (fakeland_surface_marks_v1::Error, &'static str);

fn apply(
    slot: &mut SlotState,
    request: fakeland_surface_marks_v1::Request,
) -> Result<(), RequestError> {
    use fakeland_surface_marks_v1::{Error, Request};
    let pending = &mut slot.pending;
    match request {
        Request::Frame { id_hi, id_lo } => pending.frame = Some(join_u64(id_hi, id_lo)),
        Request::InputHandled { id_hi, id_lo } => {
            pending.add_input(InputEventId(join_u64(id_hi, id_lo)));
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
            check_name(&name).map_err(|_| (Error::InvalidName, "the span name is invalid"))?;
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
            let (begin, end) = span
                .decode()
                .map_err(|_| (Error::InvalidSpan, "the span is invalid"))?;
            pending.spans.push((name, begin, end));
        }
        Request::Mark { name } => {
            check_name(&name).map_err(|_| (Error::InvalidName, "the mark name is invalid"))?;
            pending.names.push(name);
        }
        _ => {}
    }
    Ok(())
}

/// Frees the surface for a new marks object and drops its pending marks.
fn release(surface: &WlSurface) {
    if surface.is_alive() {
        with_slot(surface, |slot| *slot = SlotState::default());
    }
}
