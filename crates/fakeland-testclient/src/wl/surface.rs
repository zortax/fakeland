//! The toplevel surface and its extension objects: xdg_surface and xdg_toplevel, the viewport
//! and fractional scale objects, the fifo object and the commit timer.

use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Proxy, QueueHandle};
use wayland_protocols::wp::commit_timing::v1::client::wp_commit_timer_v1::WpCommitTimerV1;
use wayland_protocols::wp::fifo::v1::client::wp_fifo_v1::WpFifoV1;
use wayland_protocols::wp::fractional_scale::v1::client::wp_fractional_scale_v1::WpFractionalScaleV1;
use wayland_protocols::wp::viewporter::client::wp_viewport::WpViewport;
use wayland_protocols::xdg::shell::client::{xdg_surface::XdgSurface, xdg_toplevel::XdgToplevel};

use super::{Globals, MissingGlobal, Strategy};
use crate::app::App;

/// What the surface needs from the command line.
#[derive(Clone, Debug)]
pub struct SurfaceSetup<'a> {
    /// The toplevel app id.
    pub app_id: &'a str,
    /// The toplevel title.
    pub title: &'a str,
    /// The scale strategy.
    pub strategy: Strategy,
    /// Use wp_fifo_v1.
    pub fifo: bool,
    /// Use wp_commit_timer_v1.
    pub commit_timing: bool,
}

/// The surface objects.
#[derive(Debug)]
pub struct SurfaceObjects {
    /// The wl_surface.
    pub surface: WlSurface,
    /// The xdg_surface.
    pub xdg_surface: XdgSurface,
    /// The xdg_toplevel.
    pub toplevel: XdgToplevel,
    /// The viewport, for fractional scaling.
    pub viewport: Option<WpViewport>,
    /// The fractional scale object, for fractional scaling.
    pub fractional: Option<WpFractionalScaleV1>,
    /// The fifo object.
    pub fifo: Option<WpFifoV1>,
    /// The commit timer.
    pub timer: Option<WpCommitTimerV1>,
}

impl SurfaceObjects {
    /// Creates the toplevel and the extension objects that the setup asks for, and commits the
    /// initial empty state.
    pub fn create(
        globals: &Globals,
        qh: &QueueHandle<App>,
        setup: &SurfaceSetup<'_>,
    ) -> Result<Self, MissingGlobal> {
        let surface = globals.compositor.create_surface(qh, ());
        let (viewport, fractional) = match setup.strategy {
            Strategy::Fractional => {
                let viewporter = globals
                    .viewporter
                    .as_ref()
                    .ok_or(MissingGlobal("wp_viewporter"))?;
                let manager = globals
                    .fractional
                    .as_ref()
                    .ok_or(MissingGlobal("wp_fractional_scale_manager_v1"))?;
                (
                    Some(viewporter.get_viewport(&surface, qh, ())),
                    Some(manager.get_fractional_scale(&surface, qh, ())),
                )
            }
            Strategy::Integer | Strategy::Unscaled => (None, None),
        };
        let fifo = match (setup.fifo, &globals.fifo) {
            (true, Some(m)) => Some(m.get_fifo(&surface, qh, ())),
            (true, None) => return Err(MissingGlobal("wp_fifo_manager_v1")),
            (false, _) => None,
        };
        let timer = match (setup.commit_timing, &globals.commit_timing) {
            (true, Some(m)) => Some(m.get_timer(&surface, qh, ())),
            (true, None) => return Err(MissingGlobal("wp_commit_timing_manager_v1")),
            (false, _) => None,
        };
        let xdg_surface = globals.wm_base.get_xdg_surface(&surface, qh, ());
        let toplevel = xdg_surface.get_toplevel(qh, ());
        toplevel.set_app_id(setup.app_id.to_owned());
        toplevel.set_title(setup.title.to_owned());
        surface.commit();
        Ok(Self {
            surface,
            xdg_surface,
            toplevel,
            viewport,
            fractional,
            fifo,
            timer,
        })
    }

    /// Destroys every object in protocol order: extensions, toplevel, xdg_surface, surface.
    pub fn destroy(&self) {
        if let Some(timer) = &self.timer {
            timer.destroy();
        }
        if let Some(fifo) = &self.fifo {
            fifo.destroy();
        }
        if let Some(fractional) = &self.fractional {
            fractional.destroy();
        }
        if let Some(viewport) = &self.viewport {
            viewport.destroy();
        }
        self.toplevel.destroy();
        self.xdg_surface.destroy();
        self.surface.destroy();
    }

    /// Returns true when the surface supports damage in buffer coordinates.
    pub fn has_buffer_damage(&self) -> bool {
        self.surface.version() >= 4
    }

    /// Returns true when the surface supports wl_surface.set_buffer_scale.
    pub fn has_buffer_scale(&self) -> bool {
        self.surface.version() >= 3
    }
}
