//! The driver trait: the code that runs one iteration against the session.

use std::future::Future;
use std::pin::Pin;

use fakeland::{FrameCapture, ProbeReport, Session, WindowInfo};

use super::IterationContext;
use crate::error::BenchError;
use crate::input::{Observation, RunSummary};

/// The future of one driven iteration.
pub type DriveFuture<'a> =
    Pin<Box<dyn Future<Output = Result<DriverOutput, BenchError>> + Send + 'a>>;

/// What one iteration produced besides its trace.
#[derive(Clone, Debug, Default)]
pub struct DriverOutput {
    /// The scenario run.
    pub run: Option<RunSummary>,
    /// Probe reports.
    pub probes: Vec<ProbeReport>,
    /// Frame captures that the driver took.
    pub frames: Vec<FrameCapture>,
    /// Named compositor times.
    pub observations: Vec<Observation>,
    /// Free values, such as reactor output.
    pub values: Vec<serde_json::Value>,
    /// The window under test, when the driver found it, for example after a launch.
    pub target: Option<WindowInfo>,
    /// The iteration failed with this error, and its data still counts as collected.
    pub error: Option<String>,
}

/// Runs one iteration. The runner captures the trace and frames around the call.
pub trait Driver: Send + Sync {
    /// Drives one iteration.
    fn drive<'a>(&'a self, session: &'a Session, cx: &'a IterationContext) -> DriveFuture<'a>;
}

/// A driver made from an async closure over owned handles.
pub struct FnDriver<F>(F);

impl<F, Fut> Driver for FnDriver<F>
where
    F: Fn(Session, IterationContext) -> Fut + Send + Sync,
    Fut: Future<Output = Result<DriverOutput, BenchError>> + Send + 'static,
{
    fn drive<'a>(&'a self, session: &'a Session, cx: &'a IterationContext) -> DriveFuture<'a> {
        Box::pin((self.0)(session.clone(), cx.clone()))
    }
}

/// Builds a driver from an async closure, for example
/// `driver_fn(|session, cx| async move { ... })`.
pub fn driver_fn<F, Fut>(f: F) -> FnDriver<F>
where
    F: Fn(Session, IterationContext) -> Fut + Send + Sync,
    Fut: Future<Output = Result<DriverOutput, BenchError>> + Send + 'static,
{
    FnDriver(f)
}
