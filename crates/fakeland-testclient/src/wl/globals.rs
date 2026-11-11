//! The globals the client binds: the required core and shell globals and the optional
//! extensions. The registry list goes into the event log.

use wayland_client::QueueHandle;
use wayland_client::globals::GlobalList;
use wayland_client::protocol::{wl_compositor::WlCompositor, wl_seat::WlSeat, wl_shm::WlShm};
use wayland_protocols::wp::commit_timing::v1::client::wp_commit_timing_manager_v1::WpCommitTimingManagerV1;
use wayland_protocols::wp::fifo::v1::client::wp_fifo_manager_v1::WpFifoManagerV1;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_manager_v1::WpFractionalScaleManagerV1;
use wayland_protocols::wp::presentation_time::client::wp_presentation::WpPresentation;
use wayland_protocols::wp::text_input::zv3::client::zwp_text_input_manager_v3::ZwpTextInputManagerV3;
use wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use wayland_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase;

use crate::app::App;
use crate::log::GlobalInfo;

/// A required global is missing.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("the compositor offers no {0}")]
pub struct MissingGlobal(pub &'static str);

/// The bound globals.
#[derive(Debug)]
pub struct Globals {
    /// wl_compositor, up to version 6.
    pub compositor: WlCompositor,
    /// wl_shm.
    pub shm: WlShm,
    /// xdg_wm_base, up to version 6.
    pub wm_base: XdgWmBase,
    /// wl_seat, up to version 9.
    pub seat: Option<WlSeat>,
    /// wp_viewporter.
    pub viewporter: Option<WpViewporter>,
    /// wp_fractional_scale_manager_v1.
    pub fractional: Option<WpFractionalScaleManagerV1>,
    /// wp_presentation.
    pub presentation: Option<WpPresentation>,
    /// wp_fifo_manager_v1.
    pub fifo: Option<WpFifoManagerV1>,
    /// wp_commit_timing_manager_v1.
    pub commit_timing: Option<WpCommitTimingManagerV1>,
    /// zwp_text_input_manager_v3.
    pub text_input: Option<ZwpTextInputManagerV3>,
    /// Every advertised global.
    pub list: Vec<GlobalInfo>,
}

impl Globals {
    /// Binds the globals from the initial registry list.
    pub fn bind(globals: &GlobalList, qh: &QueueHandle<App>) -> Result<Self, MissingGlobal> {
        let list = globals
            .contents()
            .clone_list()
            .into_iter()
            .map(|g| GlobalInfo {
                interface: g.interface,
                version: g.version,
            })
            .collect();
        Ok(Self {
            compositor: globals
                .bind(qh, 1..=6, ())
                .map_err(|_| MissingGlobal("wl_compositor"))?,
            shm: globals
                .bind(qh, 1..=1, ())
                .map_err(|_| MissingGlobal("wl_shm"))?,
            wm_base: globals
                .bind(qh, 1..=6, ())
                .map_err(|_| MissingGlobal("xdg_wm_base"))?,
            seat: globals.bind(qh, 1..=9, ()).ok(),
            viewporter: globals.bind(qh, 1..=1, ()).ok(),
            fractional: globals.bind(qh, 1..=1, ()).ok(),
            presentation: globals.bind(qh, 1..=2, ()).ok(),
            fifo: globals.bind(qh, 1..=1, ()).ok(),
            commit_timing: globals.bind(qh, 1..=1, ()).ok(),
            text_input: globals.bind(qh, 1..=1, ()).ok(),
            list,
        })
    }
}
