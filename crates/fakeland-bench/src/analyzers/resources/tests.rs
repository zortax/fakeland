//! Resources on synthetic samples.

use fakeland_trace::Event;
use fakeland_types::ProcessId;

use super::*;
use crate::analyzer::Analyzer;
use crate::metrics::Metrics;
use crate::testing::{Synth, at_ms};

#[test]
fn summarizes_process_and_gpu_samples() {
    let mut s = Synth::new();
    for k in 0..10u64 {
        s.push(
            at_ms(100.0 * k as f64),
            Event::ProcessSample {
                pid: ProcessId(100),
                cpu_percent: 10.0 + k as f64,
                rss_bytes: 1000 + k * 10,
                threads: 4 + k as u32 % 2,
            },
        );
        s.push(
            at_ms(100.0 * k as f64),
            Event::GpuSample {
                util_percent: 50.0,
                mem_used_bytes: 2000 + k,
                mem_total_bytes: 8000,
            },
        );
    }
    let r = Resources.analyze(&s.analysis());
    let p = &r.processes[0];
    assert_eq!(p.command.as_deref(), Some("app"));
    assert_eq!(p.samples, 10);
    assert!((p.cpu_percent.unwrap().mean - 14.5).abs() < 1e-9);
    assert_eq!((p.rss_max_bytes, p.threads_max), (1090, 5));
    let g = r.gpu.as_ref().unwrap();
    assert_eq!(
        (g.samples, g.mem_used_max_bytes, g.mem_total_bytes),
        (10, 2009, 8000)
    );
    assert_eq!(r.metrics().len(), 5);
    assert_eq!(r.samples(), Some(20));
    let empty = Resources.analyze(&Synth::new().analysis());
    assert!(empty.processes.is_empty() && empty.gpu.is_none() && empty.metrics().is_empty());
    assert_eq!(empty.samples(), None);
}
