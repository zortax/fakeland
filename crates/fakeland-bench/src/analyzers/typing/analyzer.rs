//! The typing analyzer.

use fakeland_trace::{InputKind, Summary};

use super::keys::{is_modifier, key_name};
use super::{KeyLatency, TypingReport};
use crate::analyzer::Analyzer;
use crate::input::AnalysisInput;

/// Measures key-to-glyph latency from the input attribution of the trace.
#[derive(Clone, Debug, Default)]
pub struct Typing;

impl Analyzer for Typing {
    type Report = TypingReport;

    fn name(&self) -> String {
        "typing".into()
    }

    fn analyze(&self, input: &AnalysisInput) -> TypingReport {
        let glyphs: Vec<_> = input
            .timeline
            .inputs()
            .iter()
            .filter_map(|i| match i.kind {
                InputKind::Key => {
                    let (name, pressed) = key_name(&i.detail)?;
                    (pressed && !is_modifier(name)).then(|| (i, name.to_string()))
                }
                InputKind::Text => Some((i, i.detail.clone())),
                _ => None,
            })
            .collect();
        let per_key: Vec<KeyLatency> = glyphs
            .iter()
            .enumerate()
            .map(|(n, (i, key))| KeyLatency {
                input: i.id,
                key: key.clone(),
                sent: i.sent,
                handled_by: i.handled_by,
                handling_ms: i.handling_latency().map(|d| d.as_millis_f64()),
                latency_ms: i.latency().map(|d| d.as_millis_f64()),
                backlog: glyphs[..n]
                    .iter()
                    .filter(|(e, _)| e.handled_at.is_none_or(|h| h > i.sent))
                    .count(),
            })
            .collect();
        let lat: Vec<f64> = per_key.iter().filter_map(|k| k.latency_ms).collect();
        let handling: Vec<f64> = per_key.iter().filter_map(|k| k.handling_ms).collect();
        let points: Vec<(f64, f64)> = per_key
            .iter()
            .enumerate()
            .filter_map(|(i, k)| Some((i as f64, k.latency_ms?)))
            .collect();
        let span = match (per_key.first(), per_key.last()) {
            (Some(a), Some(b)) if per_key.len() > 1 => Some(b.sent.since(a.sent).as_secs_f64()),
            _ => None,
        };
        TypingReport {
            keys: per_key.len(),
            handled: per_key.iter().filter(|k| k.handled_by.is_some()).count(),
            presented: lat.len(),
            latency_ms: Summary::of(&lat),
            handling_ms: Summary::of(&handling),
            max_backlog: per_key.iter().map(|k| k.backlog).max().unwrap_or(0),
            mean_backlog: if per_key.is_empty() {
                0.0
            } else {
                per_key.iter().map(|k| k.backlog).sum::<usize>() as f64 / per_key.len() as f64
            },
            latency_slope_ms_per_key: slope(&points),
            keys_per_s: span
                .filter(|s| *s > 0.0)
                .map(|s| (per_key.len() - 1) as f64 / s),
            per_key,
        }
    }
}

/// Returns the least-squares slope of points, or `None` for fewer than three points.
fn slope(points: &[(f64, f64)]) -> Option<f64> {
    if points.len() < 3 {
        return None;
    }
    let n = points.len() as f64;
    let mx = points.iter().map(|p| p.0).sum::<f64>() / n;
    let my = points.iter().map(|p| p.1).sum::<f64>() / n;
    let sxx: f64 = points.iter().map(|p| (p.0 - mx).powi(2)).sum();
    let sxy: f64 = points.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    (sxx > 0.0).then(|| sxy / sxx)
}
