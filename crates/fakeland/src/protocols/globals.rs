//! Creation of every protocol global. The set matches a modern desktop compositor and names
//! nothing that reveals a headless session.

use fakeland_protocol::MANAGER_VERSION;
use fakeland_protocol::server::fakeland_marks_manager_v1::FakelandMarksManagerV1;
use smithay::input::SeatState;
use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::WmCapabilities;
use smithay::reexports::wayland_server::DisplayHandle;
use smithay::wayland::alpha_modifier::AlphaModifierState;
use smithay::wayland::commit_timing::CommitTimingManagerState;
use smithay::wayland::compositor::CompositorState;
use smithay::wayland::content_type::ContentTypeState;
use smithay::wayland::cursor_shape::CursorShapeManagerState;
use smithay::wayland::dmabuf::DmabufState;
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

use super::{Protocols, TearingControlState, create_text_input_global};
use crate::render::RenderBackend;
use crate::server::State;

/// The `CLOCK_MONOTONIC` id that `wp_presentation` advertises.
const CLOCK_MONOTONIC: u32 = 1;

impl Protocols {
    /// Creates every global. The dmabuf and syncobj globals exist only with a render node, and
    /// the app-mark global only when `app_marks` is set.
    pub fn new(
        dh: &DisplayHandle,
        render: &RenderBackend,
        shm_formats: Vec<smithay::reexports::wayland_server::protocol::wl_shm::Format>,
        app_marks: bool,
    ) -> Self {
        let marks = app_marks
            .then(|| dh.create_global::<State, FakelandMarksManagerV1, ()>(MANAGER_VERSION, ()));
        let mut dmabuf = DmabufState::new();
        let default_feedback = render.default_feedback();
        let scanout_feedback = render.scanout_feedback();
        let dmabuf_global = default_feedback
            .as_ref()
            .map(|feedback| dmabuf.create_global_with_default_feedback::<State>(dh, feedback));
        let syncobj = render
            .syncobj_device()
            .map(|device| DrmSyncobjState::new::<State>(dh, device));
        Self {
            compositor: CompositorState::new_v6::<State>(dh),
            xdg_shell: XdgShellState::new_with_capabilities::<State>(
                dh,
                [
                    WmCapabilities::Fullscreen,
                    WmCapabilities::Maximize,
                    WmCapabilities::Minimize,
                    WmCapabilities::WindowMenu,
                ],
            ),
            xdg_decoration: XdgDecorationState::new::<State>(dh),
            shm: ShmState::new::<State>(dh, shm_formats),
            dmabuf,
            dmabuf_global,
            default_feedback,
            scanout_feedback,
            syncobj,
            output_manager: OutputManagerState::new_with_xdg_output::<State>(dh),
            seat: SeatState::new(),
            data_device: DataDeviceState::new::<State>(dh),
            primary_selection: PrimarySelectionState::new::<State>(dh),
            presentation: PresentationState::new::<State>(dh, CLOCK_MONOTONIC),
            fifo: FifoManagerState::unmanaged::<State>(dh),
            commit_timing: CommitTimingManagerState::unmanaged::<State>(dh),
            viewporter: ViewporterState::new::<State>(dh),
            fractional_scale: FractionalScaleManagerState::new::<State>(dh),
            single_pixel: SinglePixelBufferState::new::<State>(dh),
            alpha_modifier: AlphaModifierState::new::<State>(dh),
            content_type: ContentTypeState::new::<State>(dh),
            tearing_control: TearingControlState::new(dh),
            cursor_shape: CursorShapeManagerState::new::<State>(dh),
            relative_pointer: RelativePointerManagerState::new::<State>(dh),
            pointer_constraints: PointerConstraintsState::new::<State>(dh),
            pointer_gestures: PointerGesturesState::new::<State>(dh),
            text_input: create_text_input_global(dh),
            input_method: InputMethodManagerState::new::<State, _>(dh, |_client| true),
            xdg_activation: XdgActivationState::new::<State>(dh),
            idle_inhibit: IdleInhibitManagerState::new::<State>(dh),
            keyboard_shortcuts_inhibit: KeyboardShortcutsInhibitState::new::<State>(dh),
            xdg_foreign: XdgForeignState::new::<State>(dh),
            tablet: TabletManagerState::new::<State>(dh),
            toplevel_tag: XdgToplevelTagManager::new::<State>(dh),
            system_bell: XdgSystemBellState::new::<State>(dh),
            marks,
        }
    }
}
