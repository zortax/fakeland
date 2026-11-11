//! Queries on events: the type name, the ids an event names, and the context an event sets.

use fakeland_types::{ClientId, CommitId, OutputId, ProcessId, SurfaceId, WindowId};

use crate::event::Event;

/// The entity whose state a context event sets. A trace slice keeps the context events of entities
/// that live into the slice, so the slice stays readable on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContextKey {
    /// The session itself.
    Session,
    /// An output.
    Output(OutputId),
    /// A client.
    Client(ClientId),
    /// A process.
    Process(ProcessId),
    /// A surface.
    Surface(SurfaceId),
    /// A window.
    Window(WindowId),
    /// The keyboard focus.
    Focus,
    /// The clipboard.
    Clipboard,
}

impl Event {
    /// Returns the snake_case type name, equal to the JSON `type` tag.
    pub const fn name(&self) -> &'static str {
        match self {
            Self::SessionStarted { .. } => "session_started",
            Self::OutputConfigured(_) => "output_configured",
            Self::OutputChanged(_) => "output_changed",
            Self::OutputRemoved { .. } => "output_removed",
            Self::OutputPower { .. } => "output_power",
            Self::Vblank { .. } => "vblank",
            Self::ClientConnected { .. } => "client_connected",
            Self::ClientDisconnected { .. } => "client_disconnected",
            Self::ProcessSpawned { .. } => "process_spawned",
            Self::ProcessExited { .. } => "process_exited",
            Self::SurfaceCreated { .. } => "surface_created",
            Self::SurfaceDestroyed { .. } => "surface_destroyed",
            Self::SurfaceRole { .. } => "surface_role",
            Self::WindowMapped { .. } => "window_mapped",
            Self::WindowUnmapped { .. } => "window_unmapped",
            Self::WindowTitle { .. } => "window_title",
            Self::WindowAppId { .. } => "window_app_id",
            Self::Focus { .. } => "focus",
            Self::Configure { .. } => "configure",
            Self::AckConfigure { .. } => "ack_configure",
            Self::Commit { .. } => "commit",
            Self::Ready { .. } => "ready",
            Self::Latched { .. } => "latched",
            Self::Presented { .. } => "presented",
            Self::Discarded { .. } => "discarded",
            Self::Released { .. } => "released",
            Self::FrameCallbackSent { .. } => "frame_callback_sent",
            Self::InputSent { .. } => "input_sent",
            Self::AppMark { .. } => "app_mark",
            Self::UserMark { .. } => "user_mark",
            Self::ScenarioStarted { .. } => "scenario_started",
            Self::ScenarioStep { .. } => "scenario_step",
            Self::ScenarioFinished { .. } => "scenario_finished",
            Self::ProbeStarted { .. } => "probe_started",
            Self::ProbeInput { .. } => "probe_input",
            Self::ProbeDetected { .. } => "probe_detected",
            Self::ProbeTimedOut { .. } => "probe_timed_out",
            Self::Capture { .. } => "capture",
            Self::CompositorOverhead { .. } => "compositor_overhead",
            Self::ProcessSample { .. } => "process_sample",
            Self::GpuSample { .. } => "gpu_sample",
            Self::Clipboard { .. } => "clipboard",
            Self::DeadlineWake { .. } => "deadline_wake",
            Self::ProcessDetail { .. } => "process_detail",
            Self::GpuProcessSample { .. } => "gpu_process_sample",
            Self::EnergySample { .. } => "energy_sample",
            Self::Unknown => "unknown",
        }
    }

    /// Returns the surface the event concerns.
    pub const fn surface(&self) -> Option<SurfaceId> {
        match self {
            Self::SurfaceCreated { surface, .. }
            | Self::SurfaceDestroyed { surface }
            | Self::SurfaceRole { surface, .. }
            | Self::WindowMapped { surface, .. }
            | Self::Configure { surface, .. }
            | Self::AckConfigure { surface, .. }
            | Self::Commit { surface, .. }
            | Self::Ready { surface, .. }
            | Self::Latched { surface, .. }
            | Self::Presented { surface, .. }
            | Self::Discarded { surface, .. }
            | Self::Released { surface, .. }
            | Self::FrameCallbackSent { surface, .. } => Some(*surface),
            Self::Focus { surface }
            | Self::InputSent { surface, .. }
            | Self::AppMark { surface, .. } => *surface,
            _ => None,
        }
    }

    /// Returns the commit the event concerns.
    pub const fn commit(&self) -> Option<CommitId> {
        match self {
            Self::Commit { commit, .. }
            | Self::Ready { commit, .. }
            | Self::Latched { commit, .. }
            | Self::Presented { commit, .. }
            | Self::Discarded { commit, .. }
            | Self::Released { commit, .. }
            | Self::ProbeDetected { commit, .. } => Some(*commit),
            Self::AppMark { commit, .. } => *commit,
            _ => None,
        }
    }

    /// Returns the window the event concerns.
    pub const fn window(&self) -> Option<WindowId> {
        match self {
            Self::WindowMapped { window, .. }
            | Self::WindowUnmapped { window }
            | Self::WindowTitle { window, .. }
            | Self::WindowAppId { window, .. } => Some(*window),
            _ => None,
        }
    }

    /// Returns the output the event concerns.
    pub const fn output(&self) -> Option<OutputId> {
        match self {
            Self::OutputConfigured(info) | Self::OutputChanged(info) => Some(info.output),
            Self::OutputRemoved { output }
            | Self::OutputPower { output, .. }
            | Self::Vblank { output, .. }
            | Self::DeadlineWake { output, .. }
            | Self::Latched { output, .. }
            | Self::Presented { output, .. } => Some(*output),
            Self::WindowMapped { output, .. } => *output,
            _ => None,
        }
    }

    /// Returns the client the event concerns.
    pub const fn client(&self) -> Option<ClientId> {
        match self {
            Self::ClientConnected { client, .. }
            | Self::ClientDisconnected { client }
            | Self::SurfaceCreated { client, .. } => Some(*client),
            _ => None,
        }
    }

    /// Returns the entity whose state the event sets, or `None` for an event outside the context.
    pub const fn context_key(&self) -> Option<ContextKey> {
        match self {
            Self::SessionStarted { .. } => Some(ContextKey::Session),
            Self::OutputConfigured(info) | Self::OutputChanged(info) => {
                Some(ContextKey::Output(info.output))
            }
            Self::OutputRemoved { output } | Self::OutputPower { output, .. } => {
                Some(ContextKey::Output(*output))
            }
            Self::ClientConnected { client, .. } | Self::ClientDisconnected { client } => {
                Some(ContextKey::Client(*client))
            }
            Self::ProcessSpawned { pid, .. } | Self::ProcessExited { pid, .. } => {
                Some(ContextKey::Process(*pid))
            }
            Self::SurfaceCreated { surface, .. }
            | Self::SurfaceDestroyed { surface }
            | Self::SurfaceRole { surface, .. } => Some(ContextKey::Surface(*surface)),
            Self::WindowMapped { window, .. }
            | Self::WindowUnmapped { window }
            | Self::WindowTitle { window, .. }
            | Self::WindowAppId { window, .. } => Some(ContextKey::Window(*window)),
            Self::Focus { .. } => Some(ContextKey::Focus),
            Self::Clipboard { .. } => Some(ContextKey::Clipboard),
            _ => None,
        }
    }

    /// Returns true when the event ends the life of its context entity.
    pub const fn ends_context(&self) -> bool {
        matches!(
            self,
            Self::OutputRemoved { .. }
                | Self::ClientDisconnected { .. }
                | Self::ProcessExited { .. }
                | Self::SurfaceDestroyed { .. }
                | Self::WindowUnmapped { .. }
        )
    }

    /// Returns true when the event replaces the earlier context of its entity. Focus and
    /// clipboard events keep only the latest state.
    pub const fn replaces_context(&self) -> bool {
        matches!(self, Self::Focus { .. } | Self::Clipboard { .. })
    }
}

#[cfg(test)]
mod tests {
    use fakeland_types::{InputEventId, Timestamp};

    use super::*;
    use crate::event::{InputKind, InputSource, Record};

    #[test]
    fn name_matches_the_json_tag() {
        let events = [
            Event::SurfaceDestroyed {
                surface: SurfaceId(1),
            },
            Event::InputSent {
                id: InputEventId(3),
                kind: InputKind::PointerMotion,
                surface: Some(SurfaceId(1)),
                x: Some(1.0),
                y: None,
                detail: String::new(),
                source: InputSource::Api,
            },
            Event::ProbeTimedOut {
                probe: fakeland_types::ProbeId(2),
            },
            Event::DeadlineWake {
                output: OutputId(1),
                deadline: Timestamp(5),
                woke: Timestamp(7),
            },
            Event::ProcessDetail {
                pid: ProcessId(3),
                cpu_ns_total: 1,
                ctx_switches_total: 2,
                pss_bytes: None,
                private_bytes: None,
                swap_bytes: None,
                hwm_bytes: Some(4),
            },
            Event::GpuProcessSample {
                pid: ProcessId(3),
                mem_bytes: Some(1),
                busy_ns_total: None,
                util_percent: None,
                source: "nvml".into(),
            },
            Event::EnergySample {
                domain: "cpu-package-0".into(),
                microjoules_total: 9,
            },
        ];
        for e in events {
            let json =
                serde_json::to_value(Record::new(Timestamp(1), e.clone())).unwrap_or_default();
            assert_eq!(json["type"], e.name());
        }
    }

    #[test]
    fn ids_of_a_commit() {
        let e = Event::Ready {
            surface: SurfaceId(4),
            commit: CommitId(9),
        };
        assert_eq!(e.surface(), Some(SurfaceId(4)));
        assert_eq!(e.commit(), Some(CommitId(9)));
        assert_eq!(e.window(), None);
        assert_eq!(e.context_key(), None);
    }
}
