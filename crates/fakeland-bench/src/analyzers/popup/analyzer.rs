//! The popup open analyzer.

use fakeland_trace::{Event, InputKind, Summary};
use fakeland_types::Timestamp;

use super::{PopupRecord, PopupReport};
use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;

/// Measures how fast popups and new windows open after a press.
#[derive(Clone, Debug)]
pub struct PopupOpen {
    /// New toplevels count as popups too, for apps that open dialogs.
    pub windows: bool,
}

impl Default for PopupOpen {
    fn default() -> Self {
        Self { windows: true }
    }
}

fn is_press(kind: InputKind, detail: &str) -> bool {
    match kind {
        InputKind::PointerButton | InputKind::Key => detail.contains("pressed"),
        InputKind::TouchDown => true,
        _ => false,
    }
}

impl Analyzer for PopupOpen {
    type Report = PopupReport;

    fn name(&self) -> String {
        "popup_open".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> PopupReport {
        let tl = &input.timeline;
        let presses: Vec<_> = tl
            .inputs()
            .iter()
            .filter(|i| is_press(i.kind, &i.detail))
            .collect();
        let start = input.iteration.started;
        let roles: Vec<(Timestamp, fakeland_types::SurfaceId, String)> = input
            .trace
            .records
            .iter()
            .filter(|r| r.t >= start)
            .filter_map(|r| match &r.event {
                Event::SurfaceRole { surface, role, .. }
                    if role == "xdg_popup" || (self.windows && role == "xdg_toplevel") =>
                {
                    Some((r.t, *surface, role.clone()))
                }
                _ => None,
            })
            .collect();
        let mut popups: Vec<PopupRecord> = Vec::new();
        for (mapped, surface, role) in roles {
            let trigger = presses.iter().rev().find(|p| p.sent <= mapped);
            let Some(trigger) = trigger else { continue };
            let frames = tl.frames(surface);
            let since = |t: Timestamp| t.since(trigger.sent).as_millis_f64();
            popups.push(PopupRecord {
                surface,
                role,
                input: Some(trigger.id),
                input_sent: Some(trigger.sent),
                mapped,
                map_ms: Some(since(mapped)),
                first_buffer_ms: frames
                    .iter()
                    .find(|f| f.buffer.is_some())
                    .map(|f| since(f.committed)),
                first_present_ms: frames
                    .iter()
                    .filter(|f| f.is_presented_frame())
                    .filter_map(|f| f.presented_time())
                    .min()
                    .map(since),
            });
        }
        let answered: std::collections::BTreeSet<_> =
            popups.iter().filter_map(|p| p.input).collect();
        let summary = |f: fn(&PopupRecord) -> Option<f64>| {
            let v: Vec<f64> = popups.iter().filter_map(f).collect();
            Summary::of(&v)
        };
        PopupReport {
            opened: popups.len(),
            unanswered_presses: presses.iter().filter(|p| !answered.contains(&p.id)).count(),
            map_ms: summary(|p| p.map_ms),
            first_buffer_ms: summary(|p| p.first_buffer_ms),
            first_present_ms: summary(|p| p.first_present_ms),
            popups,
        }
    }
}
