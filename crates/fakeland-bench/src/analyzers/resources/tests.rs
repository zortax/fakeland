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

fn detail(pid: u32, cpu_ns: u64, switches: u64, pss: u64) -> Event {
    Event::ProcessDetail {
        pid: ProcessId(pid),
        cpu_ns_total: cpu_ns,
        ctx_switches_total: switches,
        pss_bytes: Some(pss),
        private_bytes: Some(pss / 2),
        swap_bytes: Some(0),
        hwm_bytes: Some(pss * 2),
    }
}

fn metric(r: &ResourcesReport, name: &str) -> f64 {
    r.metrics()
        .into_iter()
        .find(|m| m.name == name)
        .unwrap_or_else(|| panic!("no metric {name}"))
        .value
}

#[test]
fn rates_from_cumulative_counters_of_the_target_tree() {
    let mut s = Synth::new();
    for k in 0..=10u64 {
        let t = at_ms(100.0 * k as f64);
        // The target tree runs 30 ms per 100 ms and switches 50 times; another tree runs flat out.
        s.push(
            t,
            detail(100, 1_000_000_000 + k * 30_000_000, 7 + k * 50, 1000 + k),
        );
        s.push(t, detail(200, k * 100_000_000, k * 1000, 9000));
        s.push(
            t,
            Event::GpuProcessSample {
                pid: ProcessId(100),
                mem_bytes: Some(4000 + k),
                busy_ns_total: Some(k * 25_000_000),
                util_percent: None,
                source: "fdinfo:amdgpu".into(),
            },
        );
        s.push(
            t,
            Event::GpuProcessSample {
                pid: ProcessId(200),
                mem_bytes: Some(50),
                busy_ns_total: None,
                util_percent: Some(10.0 * (k % 2) as f64),
                source: "nvml".into(),
            },
        );
        s.push(
            t,
            Event::EnergySample {
                domain: "cpu-package-0".into(),
                microjoules_total: k * 1_500_000,
            },
        );
        s.push(
            t,
            Event::EnergySample {
                domain: "gpu-nvml-0".into(),
                microjoules_total: 7_000_000 + k * 3_000_000,
            },
        );
    }
    let r = Resources.analyze(&s.analysis());
    assert_eq!(r.selected, [ProcessId(100)]);
    let p = &r.processes[0];
    assert_eq!(p.samples, 0);
    assert!((p.cpu_ms.unwrap() - 300.0).abs() < 1e-9);
    assert!((p.cpu_ms_per_s.unwrap() - 300.0).abs() < 1e-9);
    assert!((p.wakeups_per_s.unwrap() - 500.0).abs() < 1e-9);
    assert_eq!((p.pss_max_bytes, p.hwm_max_bytes), (Some(1010), Some(2020)));
    assert_eq!(p.gpu_mem_max_bytes, Some(4010));
    assert!((p.gpu_busy_percent.unwrap() - 25.0).abs() < 1e-9);
    assert_eq!(p.gpu_source.as_deref(), Some("fdinfo:amdgpu"));
    let other = &r.processes[1];
    assert!((other.gpu_busy_percent.unwrap() - 50.0 / 11.0).abs() < 1e-9);
    assert!((metric(&r, "cpu_ms_per_s") - 300.0).abs() < 1e-9);
    assert!((metric(&r, "cpu_ms") - 300.0).abs() < 1e-9);
    assert!((metric(&r, "wakeups_per_s") - 500.0).abs() < 1e-9);
    assert_eq!(metric(&r, "pss_max"), 1010.0);
    assert_eq!(metric(&r, "private_max"), 505.0);
    assert_eq!(metric(&r, "gpu_proc_mem_max"), 4010.0);
    assert!((metric(&r, "gpu_proc_busy_percent") - 25.0).abs() < 1e-9);
    assert!((metric(&r, "cpu_power_w") - 15.0).abs() < 1e-9);
    assert!((metric(&r, "gpu_power_w") - 30.0).abs() < 1e-9);
    assert!(r.metrics().iter().all(|m| m.name != "cpu_percent_mean"));
    assert_eq!(r.energy.len(), 2);
    assert!((r.energy[1].joules - 30.0).abs() < 1e-9);
    // Without a target, every tree counts.
    let all = Resources.analyze(&s.analysis().with_target(None));
    assert_eq!(all.selected.len(), 2);
    assert!((metric(&all, "cpu_ms_per_s") - 1300.0).abs() < 1e-9);
    assert_eq!(metric(&all, "gpu_proc_mem_max"), 4060.0);
    let back: ResourcesReport = serde_json::from_str(&serde_json::to_string(&r).unwrap()).unwrap();
    assert_eq!(back, r);
}

#[test]
fn samples_outside_the_iteration_do_not_count() {
    let mut s = Synth::new();
    s.push(at_ms(0.0), detail(100, 0, 0, 1));
    s.push(at_ms(1000.0), detail(100, 500_000_000, 0, 1));
    let input = s.analysis();
    let mut meta = input.iteration.clone();
    meta.finished = at_ms(500.0);
    let r = Resources.analyze(&input.with_meta(meta));
    assert_eq!(r.processes[0].cpu_ms_per_s, None);
    assert_eq!(r.processes[0].cpu_ms, None);
}
