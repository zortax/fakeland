//! The messages between the compositor thread and the viewer thread.

use std::sync::Arc;

use fakeland_types::OutputId;
use fakeland_vision::Image;
use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::renderer::sync::SyncPoint;

/// An output as the viewer shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct OutputView {
    /// The output.
    pub id: OutputId,
    /// The connector name.
    pub name: String,
    /// The logical size.
    pub logical: (i32, i32),
}

/// One composited picture of an output.
#[derive(Debug)]
pub enum ViewerFrame {
    /// A GPU buffer of the compositor swapchain. The viewer waits for `sync` before it shows the
    /// buffer, and reports the release of `id`.
    Dmabuf {
        /// The swapchain slot.
        id: u64,
        /// The buffer.
        dmabuf: Dmabuf,
        /// Signals when the composite is complete.
        sync: SyncPoint,
    },
    /// RGBA pixels read back on the CPU.
    Pixels(Arc<Image>),
}

/// A message to the viewer thread.
#[derive(Debug)]
pub enum ToViewer {
    /// The current outputs. The viewer opens a window per output and closes the others.
    Outputs(Vec<OutputView>),
    /// A new picture of an output.
    Frame {
        /// The output.
        output: OutputId,
        /// The picture.
        frame: ViewerFrame,
    },
    /// Swapchain slots of an output that the compositor no longer uses.
    Retire {
        /// The output.
        output: OutputId,
        /// The slots.
        ids: Vec<u64>,
    },
    /// A new window title.
    Title {
        /// The output.
        output: OutputId,
        /// The title.
        title: String,
    },
    /// Close every window and end the thread.
    Stop,
}

/// The kind of a scroll on the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AxisKind {
    /// A mouse wheel.
    Wheel,
    /// Fingers on a touchpad.
    Finger,
    /// A continuous source, for example a trackpoint.
    Continuous,
}

/// Host input in output logical coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ViewerInput {
    /// The pointer moves.
    Motion {
        /// Horizontal output coordinate.
        x: f64,
        /// Vertical output coordinate.
        y: f64,
    },
    /// A button changes state.
    Button {
        /// The evdev code.
        code: u32,
        /// True presses.
        pressed: bool,
    },
    /// One pointer frame of scrolling.
    Axis {
        /// Horizontal distance.
        dx: f64,
        /// Vertical distance.
        dy: f64,
        /// High-resolution wheel steps.
        value120: Option<(i32, i32)>,
        /// The source.
        kind: AxisKind,
        /// The scroll sequence ends.
        stop: bool,
    },
    /// A key changes state.
    Key {
        /// The evdev code on the host.
        code: u32,
        /// The keysym of the key on the base level of the host keymap.
        keysym: Option<u32>,
        /// True presses.
        pressed: bool,
    },
}

/// A message from the viewer thread.
#[derive(Debug)]
pub enum FromViewer {
    /// The viewer connected. The list holds the dmabuf formats and modifiers that the host
    /// imports; an empty list means CPU frames only.
    Started {
        /// DRM fourcc codes with modifiers.
        formats: Vec<(u32, u64)>,
    },
    /// A window shows its last picture and takes the next one.
    Ready {
        /// The output.
        output: OutputId,
        /// The width the picture shows at, in host pixels.
        width: u32,
        /// The height the picture shows at, in host pixels.
        height: u32,
    },
    /// The host no longer reads a swapchain slot.
    Released {
        /// The output.
        output: OutputId,
        /// The slot.
        id: u64,
    },
    /// The host cannot import the buffers of an output. The output switches to CPU frames.
    DmabufFailed {
        /// The output.
        output: OutputId,
    },
    /// Input in a window.
    Input {
        /// The output of the window.
        output: OutputId,
        /// The input.
        input: ViewerInput,
    },
    /// The viewer ended: a window closed or the host connection failed.
    Ended(Option<String>),
}
