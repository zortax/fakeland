//! The client state: the bound globals, the layer surface or toplevel with its popup or the
//! session lock with its lock surfaces, and the pointer position.

use wayland_client::globals::registry_queue_init;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Proxy, QueueHandle};
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_manager_v1::ExtSessionLockManagerV1;
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_surface_v1::ExtSessionLockSurfaceV1;
use wayland_protocols::ext::session_lock::v1::client::ext_session_lock_v1::ExtSessionLockV1;
use wayland_protocols::xdg::shell::client::xdg_popup::XdgPopup;
use wayland_protocols::xdg::shell::client::xdg_surface::XdgSurface;
use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use wayland_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::ZwlrLayerShellV1;
use wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_surface_v1::ZwlrLayerSurfaceV1;

use crate::args::{Args, ColorArg};
use crate::outputs::bind_outputs;
use crate::shm::{ShmError, solid_buffer};

/// A startup failure.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// The compositor is unreachable.
    #[error("connect: {0}")]
    Connect(#[from] wayland_client::ConnectError),
    /// The registry could not be read.
    #[error("globals: {0}")]
    Globals(#[from] wayland_client::globals::GlobalError),
    /// A required global is missing.
    #[error("missing global: {0}")]
    Bind(#[from] wayland_client::globals::BindError),
    /// The connection failed.
    #[error("dispatch: {0}")]
    Dispatch(#[from] wayland_client::DispatchError),
}

/// The toplevel.
#[derive(Debug)]
#[allow(
    dead_code,
    reason = "the toplevel objects live as long as the toplevel"
)]
pub struct Toplevel {
    /// The surface.
    pub surface: WlSurface,
    /// The xdg surface.
    pub xdg: XdgSurface,
    /// The toplevel.
    pub toplevel: XdgToplevel,
    /// The last configured size. 0 leaves the size to the client.
    pub size: (i32, i32),
}

/// The popup of the layer surface or the toplevel.
#[derive(Debug)]
#[allow(dead_code, reason = "the popup objects live as long as the popup")]
pub struct Popup {
    /// The surface.
    pub surface: WlSurface,
    /// The xdg surface.
    pub xdg: XdgSurface,
    /// The popup.
    pub popup: XdgPopup,
}

/// The client state.
#[derive(Debug)]
pub struct App {
    /// The command line.
    pub args: Args,
    /// `wl_compositor`.
    pub compositor: WlCompositor,
    /// `wl_shm`.
    pub shm: WlShm,
    /// `wl_seat`.
    pub seat: Option<WlSeat>,
    /// `xdg_wm_base`, for popups.
    pub wm_base: Option<XdgWmBase>,
    /// `zwlr_layer_shell_v1`.
    pub layer_shell: Option<ZwlrLayerShellV1>,
    /// `ext_session_lock_manager_v1`.
    pub lock_manager: Option<ExtSessionLockManagerV1>,
    /// Every output in registry order. The user data is the global name.
    pub outputs: Vec<WlOutput>,
    /// The layer surface.
    pub layer: Option<(WlSurface, ZwlrLayerSurfaceV1)>,
    /// The toplevel.
    pub toplevel: Option<Toplevel>,
    /// The popup.
    pub popup: Option<Popup>,
    /// The session lock.
    pub lock: Option<ExtSessionLockV1>,
    /// The lock surfaces, one per output.
    pub lock_surfaces: Vec<(WlSurface, ExtSessionLockSurfaceV1)>,
    /// The pointer position on the entered surface.
    pub pointer: (f64, f64),
    /// The entered surface.
    pub pointer_surface: Option<String>,
    /// The client unlocked and locked again.
    pub relocked: bool,
    /// The layer surface was unmapped and mapped again.
    pub remapped: bool,
    /// The lock timers run.
    pub timers_started: bool,
    /// The client is done.
    pub done: bool,
}

impl App {
    /// Connects and binds the globals.
    pub fn connect(
        args: Args,
    ) -> Result<(Self, wayland_client::EventQueue<Self>, Connection), AppError> {
        let conn = Connection::connect_to_env()?;
        let (globals, mut queue) = registry_queue_init::<Self>(&conn)?;
        let qh = queue.handle();
        let compositor: WlCompositor = globals.bind(&qh, 4..=6, ())?;
        let shm: WlShm = globals.bind(&qh, 1..=1, ())?;
        let seat: Option<WlSeat> = globals.bind(&qh, 5..=9, ()).ok();
        let outputs = bind_outputs(&globals, &qh);
        let mut app = Self {
            args,
            compositor,
            shm,
            seat,
            wm_base: globals.bind(&qh, 1..=6, ()).ok(),
            layer_shell: globals.bind(&qh, 1..=4, ()).ok(),
            lock_manager: globals.bind(&qh, 1..=1, ()).ok(),
            outputs,
            layer: None,
            toplevel: None,
            popup: None,
            lock: None,
            lock_surfaces: Vec::new(),
            pointer: (0.0, 0.0),
            pointer_surface: None,
            relocked: false,
            remapped: false,
            timers_started: false,
            done: false,
        };
        queue.roundtrip(&mut app)?;
        Ok((app, queue, conn))
    }

    /// Returns the log name of a surface: layer, toplevel, popup or lockN.
    pub fn surface_name(&self, surface: &WlSurface) -> String {
        if self.layer.as_ref().is_some_and(|(s, _)| s == surface) {
            return "layer".to_owned();
        }
        if self
            .toplevel
            .as_ref()
            .is_some_and(|t| t.surface == *surface)
        {
            return "toplevel".to_owned();
        }
        if self.popup.as_ref().is_some_and(|p| p.surface == *surface) {
            return "popup".to_owned();
        }
        self.lock_surfaces
            .iter()
            .position(|(s, _)| s == surface)
            .map_or_else(
                || format!("unknown{}", surface.id()),
                |i| format!("lock{i}"),
            )
    }

    /// Attaches a solid buffer of a size to a surface and commits it.
    pub fn draw(
        &self,
        surface: &WlSurface,
        width: u32,
        height: u32,
        color: ColorArg,
        qh: &QueueHandle<Self>,
    ) -> Result<(), ShmError> {
        let buffer = solid_buffer(&self.shm, width, height, color, qh)?;
        surface.attach(Some(&buffer), 0, 0);
        surface.damage_buffer(0, 0, width as i32, height as i32);
        surface.commit();
        Ok(())
    }
}
