//! The state of the viewer client: the host globals, the windows by output, the seat devices and
//! the channel to the compositor thread.

use std::collections::{BTreeSet, HashMap};

use fakeland_types::OutputId;
use smithay::reexports::calloop::channel::Sender;
use smithay::reexports::wayland_protocols::wp::linux_dmabuf::zv1::client::zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1;
use smithay::reexports::wayland_protocols::wp::viewporter::client::wp_viewporter::WpViewporter;
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::client::zxdg_decoration_manager_v1::ZxdgDecorationManagerV1;
use smithay::reexports::wayland_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase;
use wayland_client::backend::ObjectId;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_keyboard::WlKeyboard;
use wayland_client::protocol::wl_pointer::WlPointer;
use wayland_client::protocol::wl_shm::WlShm;
use wayland_client::protocol::wl_subcompositor::WlSubcompositor;
use wayland_client::QueueHandle;

use super::seat::AxisFrame;
use super::window::Window;
use crate::viewer::{FromViewer, ViewerInput};

/// The host globals the viewer binds.
pub struct Globals {
    /// `wl_compositor`.
    pub compositor: WlCompositor,
    /// `wl_subcompositor`.
    pub subcompositor: WlSubcompositor,
    /// `xdg_wm_base`.
    pub wm_base: XdgWmBase,
    /// `wp_viewporter`.
    pub viewporter: WpViewporter,
    /// `wl_shm`.
    pub shm: WlShm,
    /// `zwp_linux_dmabuf_v1` at version 3.
    pub dmabuf: Option<ZwpLinuxDmabufV1>,
    /// `zxdg_decoration_manager_v1`.
    pub decorations: Option<ZxdgDecorationManagerV1>,
}

/// What a host buffer belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferRef {
    /// A swapchain slot of an output.
    Slot(OutputId, u64),
    /// A shared memory buffer of an output, by id.
    Shm(OutputId, u64),
    /// The black background of a window.
    Background,
}

/// The viewer client state.
pub struct App {
    /// The channel to the compositor thread.
    pub tx: Sender<FromViewer>,
    /// The queue handle.
    pub qh: QueueHandle<App>,
    /// The globals.
    pub globals: Globals,
    /// The dmabuf formats and modifiers the host imports.
    pub formats: Vec<(u32, u64)>,
    /// Windows by output.
    pub windows: HashMap<OutputId, Window>,
    /// The output of each surface.
    pub surfaces: HashMap<ObjectId, OutputId>,
    /// The owner of each host buffer.
    pub buffers: HashMap<ObjectId, BufferRef>,
    /// The pointer.
    pub pointer: Option<WlPointer>,
    /// The keyboard.
    pub keyboard: Option<WlKeyboard>,
    /// The surface under the pointer.
    pub pointer_on: Option<ObjectId>,
    /// The surface object under the pointer.
    pub pointer_surface: Option<wayland_client::protocol::wl_surface::WlSurface>,
    /// The scroll data of the open pointer frame.
    pub axis: AxisFrame,
    /// The output with keyboard focus.
    pub key_output: Option<OutputId>,
    /// Keys the viewer pressed in the session.
    pub keys: BTreeSet<u32>,
    /// The host keymap.
    pub keymap: Option<super::keymap::HostKeymap>,
    /// Set when the viewer ends, with an error or `None` for a closed window.
    pub ended: Option<Option<String>>,
    /// The next shared memory buffer id.
    pub next_shm: u64,
}

impl App {
    /// Sends a message to the compositor thread.
    pub fn send(&self, msg: FromViewer) {
        let _ = self.tx.send(msg);
    }

    /// Returns a new shared memory buffer id.
    pub fn shm_id(&mut self) -> u64 {
        self.next_shm += 1;
        self.next_shm
    }

    /// Sends input of an output.
    pub fn input(&self, output: OutputId, input: ViewerInput) {
        self.send(FromViewer::Input { output, input });
    }
}
