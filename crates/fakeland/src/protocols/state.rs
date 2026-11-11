//! The struct that owns every protocol state of the compositor.

use smithay::input::SeatState;
use smithay::reexports::wayland_server::backend::GlobalId;
use smithay::wayland::alpha_modifier::AlphaModifierState;
use smithay::wayland::commit_timing::CommitTimingManagerState;
use smithay::wayland::compositor::CompositorState;
use smithay::wayland::content_type::ContentTypeState;
use smithay::wayland::cursor_shape::CursorShapeManagerState;
use smithay::wayland::dmabuf::{DmabufFeedback, DmabufGlobal, DmabufState};
use smithay::wayland::drm_syncobj::DrmSyncobjState;
use smithay::wayland::fifo::FifoManagerState;
use smithay::wayland::fractional_scale::FractionalScaleManagerState;
use smithay::wayland::idle_inhibit::IdleInhibitManagerState;
use smithay::wayland::input_method::InputMethodManagerState;
use smithay::wayland::keyboard_shortcuts_inhibit::KeyboardShortcutsInhibitState;
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::pointer_constraints::PointerConstraintsState;
use smithay::wayland::pointer_gestures::PointerGesturesState;
use smithay::wayland::presentation::PresentationState;
use smithay::wayland::relative_pointer::RelativePointerManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::single_pixel_buffer::SinglePixelBufferState;
use smithay::wayland::tablet_manager::TabletManagerState;
use smithay::wayland::viewporter::ViewporterState;
use smithay::wayland::xdg_activation::XdgActivationState;
use smithay::wayland::xdg_foreign::XdgForeignState;
use smithay::wayland::xdg_system_bell::XdgSystemBellState;
use smithay::wayland::xdg_toplevel_tag::XdgToplevelTagManager;

use super::TearingControlState;
use crate::server::State;

/// Every protocol state of the compositor.
#[allow(missing_docs)]
#[allow(
    dead_code,
    reason = "each state owns its global for the life of the session"
)]
pub struct Protocols {
    pub compositor: CompositorState,
    pub xdg_shell: XdgShellState,
    pub xdg_decoration: XdgDecorationState,
    pub shm: ShmState,
    pub dmabuf: DmabufState,
    pub dmabuf_global: Option<DmabufGlobal>,
    pub default_feedback: Option<DmabufFeedback>,
    pub scanout_feedback: Option<DmabufFeedback>,
    pub syncobj: Option<DrmSyncobjState>,
    pub output_manager: OutputManagerState,
    pub seat: SeatState<State>,
    pub data_device: DataDeviceState,
    pub primary_selection: PrimarySelectionState,
    pub presentation: PresentationState,
    pub fifo: FifoManagerState,
    pub commit_timing: CommitTimingManagerState,
    pub viewporter: ViewporterState,
    pub fractional_scale: FractionalScaleManagerState,
    pub single_pixel: SinglePixelBufferState,
    pub alpha_modifier: AlphaModifierState,
    pub content_type: ContentTypeState,
    pub tearing_control: TearingControlState,
    pub cursor_shape: CursorShapeManagerState,
    pub relative_pointer: RelativePointerManagerState,
    pub pointer_constraints: PointerConstraintsState,
    pub pointer_gestures: PointerGesturesState,
    pub text_input: GlobalId,
    pub input_method: InputMethodManagerState,
    pub xdg_activation: XdgActivationState,
    pub idle_inhibit: IdleInhibitManagerState,
    pub keyboard_shortcuts_inhibit: KeyboardShortcutsInhibitState,
    pub xdg_foreign: XdgForeignState,
    pub tablet: TabletManagerState,
    pub toplevel_tag: XdgToplevelTagManager,
    pub system_bell: XdgSystemBellState,
    pub marks: Option<GlobalId>,
}
