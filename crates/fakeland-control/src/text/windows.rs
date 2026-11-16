//! Text tables of windows and outputs.

use std::fmt::Write;

use fakeland::{OutputState, WindowInfo, WindowKind};

/// Renders one line per window: id, app id and title or layer state, geometry, output and state.
pub fn windows_text(windows: &[WindowInfo]) -> String {
    if windows.is_empty() {
        return "no windows\n".to_owned();
    }
    let mut out = String::new();
    for w in windows {
        let g = w.geometry;
        let mut flags = Vec::new();
        if w.focused {
            flags.push("focused");
        }
        if !w.is_mapped() {
            flags.push("unmapped");
        }
        if w.suspended {
            flags.push("suspended");
        }
        let pid = w.pid.map_or_else(|| "-".to_owned(), |p| p.0.to_string());
        let role = match (w.kind, &w.layer) {
            (WindowKind::Layer, Some(l)) => format!(
                "layer namespace={:?} layer={:?} keyboard={:?} exclusive_zone={} anchor={}",
                l.namespace,
                l.layer,
                l.keyboard,
                l.exclusive_zone,
                if l.anchor.is_empty() {
                    "none".to_owned()
                } else {
                    l.anchor.join("+")
                }
            ),
            (WindowKind::Lock, _) => "lock".to_owned(),
            _ => format!("app_id={:?} title={:?}", w.app_id, w.title),
        };
        let placement = match w.kind {
            WindowKind::Toplevel => format!(" {:?}", w.placement),
            _ => String::new(),
        };
        let _ = writeln!(
            out,
            "{} {} {}x{}+{}+{} output={} pid={}{}{}",
            w.id,
            role,
            g.w,
            g.h,
            g.x,
            g.y,
            w.output,
            pid,
            placement,
            if flags.is_empty() {
                String::new()
            } else {
                format!(" [{}]", flags.join(","))
            }
        );
    }
    out
}

/// Renders one line per output.
pub fn outputs_text(outputs: &[OutputState]) -> String {
    let mut out = String::new();
    for o in outputs {
        let _ = writeln!(
            out,
            "{} {} {}x{}@{:.3}Hz scale {} logical {}x{} at {},{}{}",
            o.id,
            o.name,
            o.mode.width,
            o.mode.height,
            o.mode.refresh.0,
            o.scale,
            o.logical_size.w,
            o.logical_size.h,
            o.position.x,
            o.position.y,
            if o.enabled { "" } else { " (off)" }
        );
    }
    out
}
