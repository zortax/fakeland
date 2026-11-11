//! wp_fifo_v1 and wp_commit_timer_v1 for the test compositor. Their state is recorded on the
//! next commit of the surface.

use fakeland_types::Timestamp;
use wayland_protocols::wp::commit_timing::v1::server::{
    wp_commit_timer_v1::{self, WpCommitTimerV1},
    wp_commit_timing_manager_v1::{self, WpCommitTimingManagerV1},
};
use wayland_protocols::wp::fifo::v1::server::{
    wp_fifo_manager_v1::{self, WpFifoManagerV1},
    wp_fifo_v1::{self, WpFifoV1},
};
use wayland_server::protocol::wl_surface::WlSurface;
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource};

use super::state::State;

impl GlobalDispatch<WpFifoManagerV1, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WpFifoManagerV1>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WpFifoManagerV1, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WpFifoManagerV1,
        request: wp_fifo_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wp_fifo_manager_v1::Request::GetFifo { id, surface } = request {
            data_init.init(id, surface);
        }
    }
}

impl Dispatch<WpFifoV1, WlSurface> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WpFifoV1,
        request: wp_fifo_v1::Request,
        surface: &WlSurface,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        let timing = &mut state.pending.entry(surface.id()).or_default().timing;
        match request {
            wp_fifo_v1::Request::SetBarrier => timing.set_barrier = true,
            wp_fifo_v1::Request::WaitBarrier => timing.wait_barrier = true,
            _ => {}
        }
    }
}

impl GlobalDispatch<WpCommitTimingManagerV1, ()> for State {
    fn bind(
        _: &mut Self,
        _: &DisplayHandle,
        _: &Client,
        resource: New<WpCommitTimingManagerV1>,
        _: &(),
        data_init: &mut DataInit<'_, Self>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WpCommitTimingManagerV1, ()> for State {
    fn request(
        _: &mut Self,
        _: &Client,
        _: &WpCommitTimingManagerV1,
        request: wp_commit_timing_manager_v1::Request,
        _: &(),
        _: &DisplayHandle,
        data_init: &mut DataInit<'_, Self>,
    ) {
        if let wp_commit_timing_manager_v1::Request::GetTimer { id, surface } = request {
            data_init.init(id, surface);
        }
    }
}

impl Dispatch<WpCommitTimerV1, WlSurface> for State {
    fn request(
        state: &mut Self,
        _: &Client,
        _: &WpCommitTimerV1,
        request: wp_commit_timer_v1::Request,
        surface: &WlSurface,
        _: &DisplayHandle,
        _: &mut DataInit<'_, Self>,
    ) {
        if let wp_commit_timer_v1::Request::SetTimestamp {
            tv_sec_hi,
            tv_sec_lo,
            tv_nsec,
        } = request
        {
            let secs = ((tv_sec_hi as u64) << 32) | tv_sec_lo as u64;
            let timing = &mut state.pending.entry(surface.id()).or_default().timing;
            timing.target = Some(Timestamp::from_secs_nanos(secs, tv_nsec));
        }
    }
}
