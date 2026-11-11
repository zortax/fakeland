//! Scenarios as a human writes them in JSON: short forms, defaults and readable durations.

use fakeland_scenario::prelude::*;

fn parse(json: &str) -> Scenario {
    Scenario::from_json(json).unwrap_or_else(|e| panic!("{e}"))
}

#[test]
fn scroll_benchmark() {
    let s = parse(
        r#"{
          "name": "scroll-glide",
          "timeout": "30s",
          "steps": [
            {"action": {"type": "wait", "until": "window", "matcher": {"type": "app_id", "app_id": "org.gnome.Nautilus"}}},
            {"action": {"type": "wait", "until": "settled", "quiet": "250ms"}},
            {"anchor": {"type": "next_vblank", "phase": "2ms"},
             "action": {"type": "pointer", "op": "move_to",
                        "pos": {"type": "window_fraction", "fx": 0.5, "fy": 0.5}}},
            {"label": "glide",
             "action": {"type": "scroll_glide",
                        "velocity": {"type": "constant", "px_per_s": 2400},
                        "duration": "2s",
                        "source": {"type": "wheel"}}}
          ]
        }"#,
    );
    assert_eq!(s.name, "scroll-glide");
    assert_eq!(s.timeout, Some(secs(30.0)));
    assert_eq!(s.steps.len(), 4);
    assert_eq!(
        s.steps[2].anchor,
        Anchor::NextVblank {
            output: None,
            phase: ms(2)
        }
    );
    let Action::ScrollGlide(g) = &s.steps[3].action else {
        panic!("scroll glide expected");
    };
    assert_eq!(g.source, ScrollSource::wheel(true));
    assert_eq!(g.rate_hz, 120.0);
    assert_eq!(g.axis, ScrollAxis::Vertical);
    assert_eq!(s.steps[3].label.as_deref(), Some("glide"));
    let Action::Wait(Wait::Settled { timeout, .. }) = &s.steps[1].action else {
        panic!("settled expected");
    };
    assert_eq!(*timeout, secs(10.0));
}

#[test]
fn typing_and_keys() {
    let s = parse(
        r#"{
          "name": "type",
          "steps": [
            {"action": {"type": "pointer", "op": "click", "at": {"type": "output", "x": 640, "y": 400}}},
            {"action": {"type": "type", "text": "hello world", "cadence": {"wpm": 120, "jitter": 0.2, "seed": 4}}},
            {"action": {"type": "key", "op": "tap", "key": "enter"}},
            {"action": {"type": "key", "op": "combo", "combo": "ctrl+shift+t"}},
            {"action": {"type": "key", "op": "press", "key": 42}},
            {"anchor": {"type": "after", "after": 50}, "action": {"type": "key", "op": "release", "key": 42}}
          ]
        }"#,
    );
    let Action::Pointer(PointerAction::Click { count, button, .. }) = &s.steps[0].action else {
        panic!("click expected");
    };
    assert_eq!((*count, *button), (1, Button::Left));
    let Action::Type { cadence, via, .. } = &s.steps[1].action else {
        panic!("type expected");
    };
    assert_eq!(cadence.wpm, 120.0);
    assert_eq!(cadence.hold, ms(20));
    assert_eq!(*via, TextVia::Keys);
    let Action::Key(KeyAction::Tap { key, .. }) = &s.steps[2].action else {
        panic!("tap expected");
    };
    assert_eq!(key, &Key::Name("Return".into()));
    let Action::Key(KeyAction::Combo { combo, .. }) = &s.steps[3].action else {
        panic!("combo expected");
    };
    assert_eq!(combo.modifiers, vec![Modifier::Ctrl, Modifier::Shift]);
    assert_eq!(s.steps[5].anchor, Anchor::After { after: ms(50) });
}

#[test]
fn ping_pong_latency_probe_with_trigger() {
    let s = parse(
        r##"{
          "name": "hover-latency",
          "steps": [
            {"action": {"type": "probe",
              "input": {"type": "pointer", "op": "move_to", "pos": {"type": "window", "x": 100, "y": 100}},
              "alternate": {"type": "pointer", "op": "move_to", "pos": {"type": "window", "x": 400, "y": 100}},
              "detector": {"type": "pixels_match",
                           "roi": {"type": "window", "rect": {"x": 80, "y": 80, "w": 40, "h": 40}},
                           "condition": {"type": "color_at", "x": 20, "y": 20, "rgba": "#3584e4"}},
              "phase": {"type": "sweep", "steps": 16},
              "repeat": 64}}
          ],
          "triggers": [
            {"on": {"type": "roi_changed", "roi": {"type": "output"}},
             "then": [{"type": "mark", "label": "changed"}],
             "limit": 100}
          ]
        }"##,
    );
    let Action::Probe(p) = &s.steps[0].action else {
        panic!("probe expected");
    };
    assert_eq!(p.repeat, 64);
    assert_eq!(p.timeout, secs(1.0));
    assert_eq!(p.settle, ms(100));
    assert_eq!(p.phase, PhaseSpec::Sweep { steps: 16 });
    let Detector::PixelsMatch {
        condition: PixelCondition::ColorAt {
            rgba, tolerance, ..
        },
        ..
    } = &p.detector
    else {
        panic!("color condition expected");
    };
    assert_eq!(*rgba, Rgba::rgb(0x35, 0x84, 0xe4));
    assert_eq!(*tolerance, 8);
    assert_eq!(s.triggers[0].limit, Some(100));
}

#[test]
fn output_and_window_changes() {
    let s = parse(
        r#"{
          "name": "modes",
          "steps": [
            {"action": {"type": "output", "op": "set_mode", "w": 2560, "h": 1440, "refresh": 143.912}},
            {"action": {"type": "output", "op": "set_scale", "scale": 1.5}},
            {"action": {"type": "output", "op": "set_transform", "transform": "90"}},
            {"action": {"type": "output", "op": "set_timing",
                        "timing": {"mode": {"type": "vrr", "min": 48, "max": 144},
                                   "callbacks": {"type": "before_vblank", "lead": "3ms"}}}},
            {"action": {"type": "window", "op": "resize_storm",
                        "from": {"w": 400, "h": 300}, "to": {"w": 1600, "h": 1000}, "duration": "1s",
                        "easing": "ease_in_out"}},
            {"action": {"type": "window", "op": "suspend", "suspended": true}},
            {"action": {"type": "capture", "max_dim": 800}},
            {"action": {"type": "clipboard", "op": "set", "text": "paste me"}},
            {"action": {"type": "sleep", "duration": "16.667ms"}}
          ]
        }"#,
    );
    let Action::Output(OutputAction::SetTiming { timing, .. }) = &s.steps[3].action else {
        panic!("timing expected");
    };
    assert_eq!(timing.mode, TimingMode::vrr(Hz(48.0), Hz(144.0)));
    assert_eq!(timing.latch_offset, Nanos::from_millis(1));
    let Action::Output(OutputAction::SetTransform { transform, .. }) = &s.steps[2].action else {
        panic!("transform expected");
    };
    assert_eq!(*transform, Transform::Rotate90);
    let Action::Clipboard(ClipboardAction::Set { mime, .. }) = &s.steps[7].action else {
        panic!("clipboard expected");
    };
    assert_eq!(mime, "text/plain;charset=utf-8");
    assert_eq!(
        s.steps[8].action,
        Action::sleep(Dur::from_nanos(16_667_000))
    );
}

#[test]
fn gestures_touch_and_repeat() {
    let s = parse(
        r#"{
          "name": "touch",
          "steps": [
            {"action": {"type": "gesture", "op": "pinch", "center": {"type": "window_fraction", "fx": 0.5, "fy": 0.5},
                        "scale_to": 2.0, "duration": "400ms"}},
            {"action": {"type": "gesture", "op": "swipe", "dx": -400, "dy": 0, "duration": "300ms"}},
            {"action": {"type": "touch", "op": "tap", "pos": {"type": "pixels", "x": 320, "y": 200, "scale": 2.0}}},
            {"action": {"type": "repeat", "count": 3, "steps": [
              {"action": {"type": "fling", "velocity": 5000}},
              {"action": {"type": "wait", "until": "frames", "n": 10}}
            ]}}
          ]
        }"#,
    );
    let Action::Gesture(Gesture::Pinch { fingers, .. }) = &s.steps[0].action else {
        panic!("pinch expected");
    };
    assert_eq!(*fingers, 2);
    let Action::Gesture(Gesture::Swipe { fingers, .. }) = &s.steps[1].action else {
        panic!("swipe expected");
    };
    assert_eq!(*fingers, 3);
    let Action::Repeat { steps, .. } = &s.steps[3].action else {
        panic!("repeat expected");
    };
    let Action::Fling(f) = &steps[0].action else {
        panic!("fling expected");
    };
    assert_eq!(f.time_constant, ms(325));
}

#[test]
fn invalid_scenarios_name_the_location() {
    let err = Scenario::from_json(
        r#"{"name": "bad", "steps": [
            {"action": {"type": "mark", "label": "ok"}},
            {"action": {"type": "repeat", "count": 2, "steps": [
              {"action": {"type": "glide", "path": {"type": "line",
                 "from": {"type": "output", "x": 0, "y": 0}, "to": {"type": "output", "x": 1, "y": 1}},
               "duration": "1s", "rate_hz": 0}}]}}]}"#,
    )
    .err()
    .map(|e| e.to_string())
    .unwrap_or_default();
    assert!(err.starts_with("steps[1].steps[0]"), "{err}");
    let err = Scenario::from_json(r#"{"name": "bad", "steps": [{"action": {"type": "warp"}}]}"#);
    assert!(matches!(err, Err(ScenarioError::Json(_))));
    let err = Scenario::from_json(
        r#"{"name": "bad", "steps": [], "triggers": [
            {"on": {"type": "input_sent"}, "then": [{"type": "sleep", "duration": "1s"}]}]}"#,
    );
    assert!(matches!(err, Err(ScenarioError::Invalid { .. })));
    let err = Scenario::from_json(
        r#"{"name": "bad", "steps": [{"action": {"type": "key", "op": "combo", "combo": "hyper+x"}}]}"#,
    );
    assert!(matches!(err, Err(ScenarioError::Json(_))));
}
