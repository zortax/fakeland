//! Probes on layer surfaces with the layer client: a buffer that a layer commits before it maps
//! is tested once the layer maps. Run with `cargo test -p fakeland -- --ignored`.

mod support;

use fakeland::scenario::{Action, Detector, PointerAction, Position, ProbeSpec, Roi};
use fakeland_scenario::Button;
use fakeland_types::Rect;
use support::shell::{layer, session, wait_layer, wait_pixel};

#[test]
#[ignore = "runs the layer client"]
fn the_first_buffer_of_a_new_mapping_is_the_response() {
    let _serial = support::serial();
    let s = session(1280, 720);
    let app = s
        .blocking_launch(layer(&[
            "--namespace",
            "remap",
            "--anchor",
            "top,left",
            "--size",
            "200x100",
            "--margin",
            "100,0,0,100",
            "--color",
            "#3060c0",
            "--popup-color",
            "#e06020",
            "--remap-on-press",
        ]))
        .expect("launch");
    wait_layer(&s, &app, "remap");
    wait_pixel(&s, 150, 150, [0x30, 0x60, 0xc0]);

    let spec = ProbeSpec::new(
        Action::Pointer(PointerAction::click(
            Some(Position::output(150.0, 150.0)),
            Button::Left,
        )),
        Detector::PixelsChanged {
            roi: Roi::output_rect(None, Rect::new(110, 110, 100, 60)),
            threshold: 0.5,
        },
    )
    .repeat(1);
    let report = s.blocking_probe(spec).expect("probe");
    assert_eq!(
        report.timeouts(),
        0,
        "the probe finds the buffer of the new mapping: {}",
        app.stdout()
    );
    let result = &report.results[0];
    assert_eq!(
        result.presented_commit, result.detected_commit,
        "the buffer of the new mapping is the one presented"
    );
    let latched = result.latched.expect("latched");
    let presented = result.presented.expect("presented");
    assert!(
        presented.0 > latched.0 && presented.0 - latched.0 < 20_000_000,
        "the response is presented at the refresh that maps the layer"
    );
    wait_pixel(&s, 150, 150, [0xe0, 0x60, 0x20]);
}
