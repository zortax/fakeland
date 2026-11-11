//! The human-readable form of a bench report: the host, the settings, the validity checks and a
//! metric table per section.

use std::fmt::Write;

use fakeland_timing::TimingMode;

use super::table::num;
use super::{BenchReport, Section, Table};
use crate::metrics::Better;

impl BenchReport {
    /// Renders the report as text tables.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let preset = self
            .preset
            .as_ref()
            .map(|p| format!(" (preset {p})"))
            .unwrap_or_default();
        let _ = writeln!(out, "bench {}{preset}", self.name);
        let e = &self.environment;
        let host: Vec<String> = [
            ("gpu", &e.gpu),
            ("driver", &e.driver),
            ("kernel", &e.kernel),
            ("cpu", &e.cpu),
            ("governor", &e.governor),
            ("renderer", &e.renderer),
        ]
        .into_iter()
        .filter(|(_, v)| !v.is_empty())
        .map(|(k, v)| format!("{k} {v}"))
        .collect();
        if !host.is_empty() {
            let _ = writeln!(out, "host: {}", host.join(" | "));
        }
        for o in &self.config.outputs {
            let _ = writeln!(
                out,
                "output {}: {}x{} @ {:.3} Hz, scale {}, timing {}",
                o.name,
                o.mode.width,
                o.mode.height,
                o.mode.refresh.0,
                o.scale,
                timing_label(&o.timing.mode)
            );
        }
        let c = &self.config;
        let _ = writeln!(
            out,
            "iterations: {} measured + {} warmup per variant, settle {}, duration {:.2} s",
            c.iterations,
            c.warmup,
            c.settle,
            self.duration().as_secs_f64()
        );
        if !c.variants.is_empty() {
            let _ = writeln!(out, "variants: {}", c.variants.join(", "));
        }
        let _ = writeln!(
            out,
            "validity: {}",
            if self.validity.valid {
                "valid"
            } else {
                "INVALID"
            }
        );
        for check in &self.validity.checks {
            let _ = writeln!(
                out,
                "  [{}] {}: {}",
                check.status.label(),
                check.name,
                check.message
            );
        }
        for section in &self.sections {
            out.push('\n');
            out.push_str(&section_text(section));
        }
        out
    }
}

fn section_text(s: &Section) -> String {
    let mut out = String::new();
    let samples = s
        .samples
        .map(|n| format!(", {n} samples"))
        .unwrap_or_default();
    let _ = writeln!(out, "== {}{samples} ==", s.name);
    if let Some(e) = &s.error {
        let _ = writeln!(out, "error: {e}");
    }
    let variants = s.metrics.iter().any(|m| m.variant.is_some());
    let mut header = vec!["metric"];
    if variants {
        header.push("variant");
    }
    header.extend([
        "unit", "mean", "median", "95% ci", "min", "max", "n", "better",
    ]);
    let mut t = Table::new(header);
    for m in &s.metrics {
        let mut row = vec![m.name.clone()];
        if variants {
            row.push(m.variant.clone().unwrap_or_default());
        }
        row.extend([
            m.unit.label().to_string(),
            num(m.mean),
            num(m.median),
            m.ci.map(|c| format!("{} .. {}", num(c.lo), num(c.hi)))
                .unwrap_or_else(|| "-".into()),
            num(m.min),
            num(m.max),
            m.n.to_string(),
            better(m.better).to_string(),
        ]);
        t.row(row);
    }
    if t.is_empty() {
        out.push_str("no metrics\n");
    } else {
        out.push_str(&t.render());
    }
    for p in s.pooled.iter().filter(|p| !p.metrics.is_empty()) {
        let label = p
            .variant
            .as_ref()
            .map(|v| format!(" {v}"))
            .unwrap_or_default();
        let n = p
            .samples
            .map(|n| format!(", {n} samples"))
            .unwrap_or_default();
        let _ = writeln!(out, "pooled{label}{n}:");
        let mut t = Table::new(["metric", "unit", "value"]);
        for m in &p.metrics {
            t.row([m.name.clone(), m.unit.label().to_string(), num(m.value)]);
        }
        out.push_str(&t.render());
    }
    out
}

fn timing_label(mode: &TimingMode) -> String {
    match *mode {
        TimingMode::Fixed { refresh } => format!("fixed {refresh}"),
        TimingMode::Vrr { min, max } => format!("vrr {min} to {max}"),
        TimingMode::Unthrottled => "unthrottled".into(),
        TimingMode::Manual { nominal } => format!("manual {nominal}"),
    }
}

fn better(b: Better) -> &'static str {
    match b {
        Better::Lower => "lower",
        Better::Higher => "higher",
        Better::Neither => "-",
    }
}
