//! Resource sampling: CPU of busy shell loops with a known load, and plausible samples of vkcube
//! and alacritty. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use std::collections::BTreeMap;

use fakeland::{
    Event, Fakeland, LaunchSpec, OutputConfig, RendererChoice, SamplingConfig, Session, Trace,
    WindowRef,
};
use fakeland_types::{Hz, Nanos, ProcessId};

#[derive(Debug, Default)]
struct Stats {
    cpu: Vec<f64>,
    rss_max: u64,
    threads_max: u32,
}

fn per_pid(trace: &Trace) -> BTreeMap<ProcessId, Stats> {
    let mut out: BTreeMap<ProcessId, Stats> = BTreeMap::new();
    for r in &trace.records {
        if let Event::ProcessSample {
            pid,
            cpu_percent,
            rss_bytes,
            threads,
        } = &r.event
        {
            let s = out.entry(*pid).or_default();
            s.cpu.push(*cpu_percent);
            s.rss_max = s.rss_max.max(*rss_bytes);
            s.threads_max = s.threads_max.max(*threads);
        }
    }
    out
}

fn mean(v: &[f64]) -> f64 {
    v.iter().sum::<f64>() / v.len().max(1) as f64
}

fn sampled_session(renderer: RendererChoice) -> Session {
    Fakeland::builder()
        .output(OutputConfig::new("DP-1").mode(1280, 720, Hz(60.0)))
        .renderer(renderer)
        .sampling(SamplingConfig::every(Nanos::from_millis(100)))
        .start()
        .expect("start")
}

fn sleep(d: std::time::Duration) {
    std::thread::sleep(d);
}

#[test]
#[ignore = "runs busy shell loops for three seconds"]
fn busy_loops_read_one_core_per_process() {
    let _guard = support::serial();
    let session = sampled_session(RendererChoice::Pixman);
    let one = session
        .blocking_launch(LaunchSpec::new("sh").args(["-c", "while :; do :; done"]))
        .expect("launch");
    let two = session
        .blocking_launch(
            LaunchSpec::new("sh")
                .args(["-c", "(while :; do :; done) & (while :; do :; done) & wait"]),
        )
        .expect("launch");
    let idle = session
        .blocking_launch(LaunchSpec::new("sleep").arg("30"))
        .expect("launch");
    sleep(std::time::Duration::from_millis(2600));
    let snapshot = session.blocking_resources().expect("resources");
    eprint!("{}", snapshot.to_text());
    let trace = session.blocking_trace_snapshot().expect("trace");
    let stats = per_pid(&trace);
    for (pid, s) in &stats {
        eprintln!(
            "pid {}: {} samples, cpu mean {:.1}%, rss max {:.1} MiB, threads max {}",
            pid.0,
            s.cpu.len(),
            mean(&s.cpu),
            s.rss_max as f64 / 1048576.0,
            s.threads_max
        );
    }
    let one_s = &stats[&one.pid()];
    let two_s = &stats[&two.pid()];
    let idle_s = &stats[&idle.pid()];
    assert!(one_s.cpu.len() >= 20, "{} samples", one_s.cpu.len());
    let (m1, m2, m0) = (mean(&one_s.cpu), mean(&two_s.cpu), mean(&idle_s.cpu));
    assert!((85.0..=110.0).contains(&m1), "one loop: {m1}");
    assert!((170.0..=215.0).contains(&m2), "two loops: {m2}");
    assert!(m0 < 2.0, "sleep: {m0}");
    assert!(one_s.rss_max > 0 && one_s.threads_max == 1);
    let tree = snapshot
        .processes
        .iter()
        .find(|p| p.pid == two.pid())
        .expect("snapshot of the two-loop tree");
    assert_eq!(tree.processes, 3, "sh plus two subshells");
    assert!(snapshot.enabled && snapshot.time.is_some());

    // A tree ends after its root exits, and its samples stop.
    one.kill().unwrap();
    let _ = one.blocking_wait();
    sleep(std::time::Duration::from_millis(400));
    let snapshot = session.blocking_resources().expect("resources");
    assert!(!snapshot.processes.iter().any(|p| p.pid == one.pid()));
    session.blocking_shutdown().unwrap();
}

#[test]
#[ignore = "runs vkcube and alacritty on the GPU"]
fn gpu_apps_have_plausible_samples() {
    if !support::have("vkcube") {
        eprintln!("vkcube is missing");
        return;
    }
    let _guard = support::serial();
    let session = sampled_session(RendererChoice::Gles);
    let cube = session
        .blocking_launch(LaunchSpec::new("vkcube").args(["--wsi", "wayland"]))
        .expect("vkcube");
    let term = support::have("alacritty").then(|| {
        session
            .blocking_launch(LaunchSpec::new("alacritty").args(["-e", "sh", "-c", "sleep 30"]))
            .expect("alacritty")
    });
    session
        .blocking_wait_window(WindowRef::Any, Nanos::from_secs(10))
        .expect("window");
    sleep(std::time::Duration::from_millis(3000));
    let snapshot = session.blocking_resources().expect("resources");
    eprint!("{}", snapshot.to_text());
    let trace = session.blocking_trace_snapshot().expect("trace");
    let stats = per_pid(&trace);
    let cube_s = &stats[&cube.pid()];
    let cube_cpu = mean(&cube_s.cpu);
    eprintln!(
        "vkcube: cpu {cube_cpu:.1}%, rss {:.1} MiB, threads {}",
        cube_s.rss_max as f64 / 1048576.0,
        cube_s.threads_max
    );
    assert!(cube_s.cpu.len() >= 20);
    assert!((0.5..400.0).contains(&cube_cpu), "vkcube cpu {cube_cpu}");
    assert!(cube_s.rss_max > 20 << 20, "vkcube rss {}", cube_s.rss_max);
    assert!(cube_s.threads_max >= 2);
    if let Some(term) = &term {
        let t = &stats[&term.pid()];
        let cpu = mean(&t.cpu);
        eprintln!(
            "alacritty: cpu {cpu:.1}%, rss {:.1} MiB, threads {}",
            t.rss_max as f64 / 1048576.0,
            t.threads_max
        );
        assert!(cpu < 100.0, "idle terminal cpu {cpu}");
        assert!(t.rss_max > 10 << 20);
        assert!(t.threads_max >= 2);
    }
    let gpu: Vec<(f64, u64)> = trace
        .records
        .iter()
        .filter_map(|r| match &r.event {
            Event::GpuSample {
                util_percent,
                mem_used_bytes,
                ..
            } => Some((*util_percent, *mem_used_bytes)),
            _ => None,
        })
        .collect();
    eprintln!(
        "gpu source {:?}: {} samples, last {:?}",
        snapshot.gpu_source,
        gpu.len(),
        gpu.last()
    );
    if cfg!(feature = "nvml") {
        assert_eq!(snapshot.gpu_source.as_deref(), Some("nvml"));
        assert!(gpu.len() >= 20);
        assert!(gpu.iter().all(|(u, m)| (0.0..=100.0).contains(u) && *m > 0));
    }
    let timeline = fakeland_trace::FrameTimeline::build(&trace);
    let mut perfetto = Vec::new();
    fakeland_trace::perfetto::write_chrome_trace(&trace, &timeline, &mut perfetto).unwrap();
    let json = String::from_utf8(perfetto).unwrap();
    assert!(json.contains("\"cpu %\"") && json.contains("rss MiB"));
    session.blocking_shutdown().unwrap();
}
