//! Startup on a synthetic launch.

use fakeland::{ProbeReport, ProbeResult, ProbeStages};
use fakeland_trace::Event;
use fakeland_types::{ClientId, Nanos, ProcessId, SurfaceId, WindowId};

use super::*;
use crate::analyzer::Analyzer;
use crate::input::Observation;
use crate::metrics::Metrics;
use crate::testing::{CommitOpts, Synth, at_ms};

#[test]
fn milestones_count_from_the_spawn() {
    let mut s = Synth::new();
    s.push(
        at_ms(100.0),
        Event::ProcessSpawned {
            pid: ProcessId(200),
            command: "slow-app --startup-delay 250ms".into(),
        },
    );
    s.push(
        at_ms(350.0),
        Event::ClientConnected {
            client: ClientId(2),
            pid: Some(ProcessId(200)),
        },
    );
    let surface = SurfaceId(5);
    s.push(
        at_ms(360.0),
        Event::SurfaceCreated {
            surface,
            client: ClientId(2),
        },
    );
    s.push(
        at_ms(360.0),
        Event::SurfaceRole {
            surface,
            role: "xdg_toplevel".into(),
            parent: None,
        },
    );
    s.push(
        at_ms(380.0),
        Event::WindowMapped {
            window: WindowId(2),
            surface,
            app_id: "slow".into(),
            title: "Slow".into(),
            output: None,
        },
    );
    // The initial commit has no buffer. Its presentation shows nothing.
    let empty = s.commit(
        at_ms(390.0),
        CommitOpts {
            surface,
            buffer: false,
            ..CommitOpts::default()
        },
    );
    s.present(empty, 24);
    let c = s.commit(
        at_ms(400.0),
        CommitOpts {
            surface,
            ..CommitOpts::default()
        },
    );
    s.present(c, 25);
    let probe = ProbeReport {
        label: None,
        detector: "any_damage".into(),
        results: vec![ProbeResult {
            presented: Some(at_ms(700.0)),
            stages: ProbeStages {
                total: Some(Nanos::from_millis(20)),
                ..ProbeStages::default()
            },
            ..ProbeResult::default()
        }],
    };
    let input = s
        .analysis()
        .with_target(None)
        .with_observations(vec![Observation::new(VISUALLY_COMPLETE, at_ms(600.0))])
        .with_probes(vec![probe]);
    let r = Startup {
        command: Some("slow-app".into()),
    }
    .analyze(&input);
    assert_eq!(r.pid, Some(ProcessId(200)));
    assert_eq!(r.window, Some(WindowId(2)));
    let near = |v: Option<f64>, want: f64| (v.unwrap() - want).abs() < 1e-3;
    assert!(near(r.connected_ms, 250.0));
    assert!(near(r.first_surface_ms, 260.0));
    assert!(near(r.toplevel_ms, 280.0));
    assert!(near(r.first_buffer_ms, 300.0));
    assert!(near(r.first_present_ms, 25.0 * 1000.0 / 60.0 - 100.0));
    assert!(near(r.visually_complete_ms, 500.0));
    assert!(near(r.responsive_ms, 600.0));
    assert!(near(r.first_input_latency_ms, 20.0));
    assert!(!r.exited_early);
    assert_eq!(r.metrics().len(), 8);
    let first = Startup::default().analyze(&input);
    assert_eq!(
        first.pid,
        Some(ProcessId(100)),
        "the first spawn counts without a filter"
    );
}
