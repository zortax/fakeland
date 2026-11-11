//! The startup analyzer.

use fakeland_trace::Event;
use fakeland_types::Timestamp;

use super::StartupReport;
use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;

/// The observation name of the time the window content became stable.
pub const VISUALLY_COMPLETE: &str = "visually_complete";

/// The observation name of the time the first probe started.
pub const RESPONSIVE: &str = "responsive_probe";

/// Measures app startup from the first process spawn of the iteration.
#[derive(Clone, Debug, Default)]
pub struct Startup {
    /// Only a spawn whose command contains this text counts; `None` takes the first spawn.
    pub command: Option<String>,
}

impl Analyzer for Startup {
    type Report = StartupReport;

    fn name(&self) -> String {
        "startup".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> StartupReport {
        let records = &input.trace.records;
        let spawn = records.iter().find_map(|r| match &r.event {
            Event::ProcessSpawned { pid, command }
                if self
                    .command
                    .as_ref()
                    .is_none_or(|c| command.contains(c.as_str())) =>
            {
                Some((r.t, *pid, command.clone()))
            }
            _ => None,
        });
        let Some((spawned, pid, command)) = spawn else {
            return StartupReport::default();
        };
        let tl = &input.timeline;
        let since = |t: Timestamp| t.since(spawned).as_millis_f64();
        let after = |t: Timestamp| t >= spawned;
        let clients: Vec<_> = tl
            .clients()
            .filter(|c| c.connected.is_some_and(after))
            .collect();
        let client = clients
            .iter()
            .find(|c| c.pid == Some(pid))
            .or_else(|| clients.first())
            .copied();
        let first_surface = tl
            .surfaces()
            .filter(|s| client.is_some_and(|c| s.client == Some(c.client)))
            .filter_map(|s| s.created)
            .min();
        let window = input
            .target
            .as_ref()
            .and_then(|t| tl.window(t.window))
            .or_else(|| {
                tl.windows().filter(|w| after(w.mapped)).find(|w| {
                    client.is_some_and(|c| {
                        tl.surface(w.surface).and_then(|s| s.client) == Some(c.client)
                    })
                })
            })
            .or_else(|| tl.windows().find(|w| after(w.mapped)));
        let tree = window
            .map(|w| tl.window_surfaces(w.window))
            .unwrap_or_default();
        let frames = || tree.iter().flat_map(|s| tl.frames(*s).iter());
        let first_buffer = frames()
            .filter(|f| f.buffer.is_some())
            .map(|f| f.committed)
            .min();
        let first_present = frames()
            .filter(|f| f.is_presented_frame())
            .filter_map(|f| f.presented_time())
            .min();
        let probe = input.probe_results().find(|r| r.presented.is_some());
        let exited = tl.process(pid).and_then(|p| p.exited);
        StartupReport {
            pid: Some(pid),
            command: Some(command),
            spawned: Some(spawned),
            window: window.map(|w| w.window),
            connected_ms: client.and_then(|c| c.connected).map(since),
            first_surface_ms: first_surface.map(since),
            toplevel_ms: window.map(|w| since(w.mapped)),
            first_buffer_ms: first_buffer.map(since),
            first_present_ms: first_present.map(since),
            visually_complete_ms: input.observation(VISUALLY_COMPLETE).map(since),
            responsive_ms: probe.and_then(|p| p.presented).map(since),
            first_input_latency_ms: probe
                .and_then(|p| p.stages.total)
                .map(|n| n.as_millis_f64()),
            exited_early: exited.is_some_and(|e| window.is_none_or(|w| w.mapped > e)),
        }
    }
}
