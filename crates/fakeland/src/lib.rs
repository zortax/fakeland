//! Headless Wayland compositor for benchmarks and scripted app control. A session fakes outputs and
//! display timing, launches apps, captures frames and records traces.
//!
//! [`Fakeland::start`] spawns the compositor thread and returns a [`Session`]. The session is a
//! `Clone + Send + Sync` handle with async methods and `blocking_*` variants.
//!
//! Input, scenarios, waits, triggers, probes and reactors all run on the compositor thread, so
//! every timestamp is a compositor timestamp: [`Session::inject`] and the input helpers send
//! events at once, [`Session::run`] runs a [`scenario::Scenario`] and returns a [`RunHandle`],
//! [`Session::probe`] measures input latency, [`Session::install_reactor`] installs Rust code
//! that reacts to every trace record, and [`Session::begin_frame_capture`] keeps an image of
//! every commit or presented frame of a window.
//!
//! [`Session::record`] writes a video of an output or a window whose frames follow the fake
//! vblanks. [`SessionConfig::sampling`] samples CPU, memory and threads of every launched app
//! and the GPU into the trace, and [`Session::resources`] returns the latest samples. With the
//! `viewer` feature, [`SessionConfig::viewer`] shows each output in a window on the host desktop
//! and forwards the input of that window into the session. The `nvml` feature adds GPU samples
//! of NVIDIA GPUs.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod config;

mod a11y;

mod actions;
mod capture;
mod display;
mod framecap;
mod frames;
mod input;
mod launch;
mod marks;
mod outputs;
mod probes;
mod protocols;
pub mod reactor;
mod recording;
mod render;
mod runner;
mod sampling;
mod selection;
mod server;
mod session;
mod shell;
mod triggers;
mod util;
mod video;
#[cfg(feature = "viewer")]
mod viewer;
mod waits;

pub use a11y::{A11yTarget, Accessibility, ClickOutcome, ClickVia, TextMethod};
pub use actions::{ActionError, ActionOutcome};
pub use capture::{CaptureError, CaptureOptions, Screenshot};
pub use config::{
    Composite, ConfigError, DbusMode, Decorations, InputConfig, Keymap, ModeConfig, OutputConfig,
    OutputPosition, PhysicalSize, PointerDevice, RendererChoice, Repeat, SamplingConfig,
    SessionConfig, UnmappedKeys, WheelKind, WindowPolicy,
};
pub use display::RunValidity;
pub use framecap::{CaptureSpec, CapturedFrame, FrameCapture, FrameCaptureId};
pub use input::{InjectError, SentInput};
pub use launch::{App, ExitInfo, LaunchError, LaunchSpec};
pub use outputs::{OutputChange, OutputState};
pub use probes::{ProbeReport, ProbeResult, ProbeStages};
pub use reactor::{Reactor, ReactorCx, ReactorHandle, ReactorId, reactor_fn};
pub use recording::{Subscription, SubscriptionItem};
pub use runner::{
    CaptureResult, RunError, RunErrorKind, RunOptions, RunResult, StepTiming, TriggerFiring,
};
pub use sampling::{EnergyReading, GpuReading, ProcessTreeSample, ResourceSnapshot};
pub use server::ClientInfo;
pub use session::{
    Fakeland, FakelandBuilder, RunHandle, Session, SessionError, StartError, WindowOp,
};
pub use shell::{MatchError, Placement, WindowInfo};
pub use util::block_on;
pub use video::{
    FrameRate, RecordError, RecordSpec, RecordTarget, Recording, RecordingId, RecordingInfo,
    RecordingReport, VideoFormat,
};
pub use waits::{ConditionError, WaitError, WaitOutput};

pub use fakeland_a11y as accessibility;
pub use fakeland_scenario as scenario;
pub use fakeland_scenario::{CaptureTarget, Roi, Transform, WindowRef};
pub use fakeland_timing::{CallbackPolicy, Perturbation, TimingConfig, TimingMode};
pub use fakeland_trace::{CaptureId, Environment, Event, Record, Trace};
pub use fakeland_vision::Image;
