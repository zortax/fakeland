//! The human-readable form of a comparison.

use std::fmt::Write;

use super::{Comparison, Verdict};
use crate::report::Table;
use crate::report::table::num;

impl Comparison {
    /// Renders the comparison as a table of every metric with a delta.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "compare {} (baseline) -> {}: {:?}, {} regressed, {} improved",
            self.baseline, self.current, self.outcome, self.regressions, self.improvements
        );
        for w in &self.warnings {
            let _ = writeln!(out, "warning: {w}");
        }
        let mut t = Table::new([
            "section", "metric", "variant", "unit", "baseline", "current", "delta", "change",
            "verdict",
        ]);
        for m in &self.metrics {
            let opt = |v: Option<f64>| v.map(num).unwrap_or_else(|| "-".into());
            t.row([
                m.section.clone(),
                m.name.clone(),
                m.variant.clone().unwrap_or_default(),
                m.unit.label().to_string(),
                opt(m.baseline),
                opt(m.current),
                m.delta
                    .map(|d| format!("{d:+.3}"))
                    .unwrap_or_else(|| "-".into()),
                m.relative
                    .map(|r| format!("{:+.1}%", r * 100.0))
                    .unwrap_or_else(|| "-".into()),
                verdict(m.verdict).to_string(),
            ]);
        }
        out.push_str(&t.render());
        out
    }
}

fn verdict(v: Verdict) -> &'static str {
    match v {
        Verdict::Improved => "improved",
        Verdict::Regressed => "REGRESSED",
        Verdict::Unchanged => "unchanged",
        Verdict::Inconclusive => "inconclusive",
        Verdict::Changed => "changed",
        Verdict::Added => "added",
        Verdict::Removed => "removed",
        Verdict::Ignored => "ignored",
    }
}
