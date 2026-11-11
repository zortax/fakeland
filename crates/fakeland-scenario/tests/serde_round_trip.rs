//! Serde round trips for every enum variant of the scenario language.

use std::fmt::Debug;

use fakeland_scenario::prelude::*;
use serde::Serialize;
use serde::de::DeserializeOwned;

fn rt<T: Serialize + DeserializeOwned + PartialEq + Debug>(value: &T) {
    let json = serde_json::to_string(value).unwrap_or_else(|e| panic!("{e}"));
    let back: T = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{json}: {e}"));
    assert_eq!(&back, value, "{json}");
}

fn win() -> WindowRef {
    WindowRef::app_id("org.example.App")
}

fn positions() -> Vec<Position> {
    vec![
        Position::output(1.5, 2.0),
        Position::on_output(OutputId(2), 3.0, 4.0),
        Position::window(win(), 10.0, 20.0),
        Position::fraction(WindowRef::title_regex("^Doc.*"), 0.25, 0.75),
        Position::Pixels {
            output: Some(OutputId(1)),
            x: 100.0,
            y: 50.0,
            scale: 2.5,
        },
    ]
}

fn paths() -> Vec<Path> {
    let p = Position::output(0.0, 0.0);
    let q = Position::output(100.0, 50.0);
    vec![
        Path::line(p.clone(), q.clone()),
        Path::polyline([p.clone(), q.clone(), p.clone()]),
        Path::bezier(p.clone(), q.clone(), p.clone(), q.clone()),
        Path::circle(q, 30.0, 1.5),
    ]
}

fn rois() -> Vec<Roi> {
    vec![
        Roi::window(win()),
        Roi::window_rect(WindowId(3), Rect::new(1, 2, 30, 40)),
        Roi::output(None),
        Roi::output_rect(Some(OutputId(1)), Rect::new(0, 0, 10, 10)),
    ]
}

fn conditions() -> Vec<PixelCondition> {
    vec![
        PixelCondition::changed(),
        PixelCondition::ColorAt {
            x: 3,
            y: 4,
            rgba: Rgba::rgb(255, 0, 0),
            tolerance: 5,
        },
        PixelCondition::MeanColor {
            rgba: Rgba([1, 2, 3, 4]),
            tolerance: 0,
        },
        PixelCondition::MatchesImage {
            png_base64: "iVBORw0KGgo=".into(),
            max_mean_delta: 1.5,
        },
    ]
}

fn waits() -> Vec<Wait> {
    vec![
        Wait::window(win()),
        Wait::settled(Some(WindowRef::Any), ms(300)),
        Wait::frames(5, None),
        Wait::presented(Some(WindowRef::Focused)),
        Wait::pixels_stable(Roi::output(None), 4),
        Wait::pixels_match(Roi::window(win()), PixelCondition::changed()).with_timeout(ms(500)),
        Wait::app_mark("ready"),
    ]
}

fn actions() -> Vec<Action> {
    let p = Position::output(5.0, 6.0);
    let mut out: Vec<Action> = vec![
        PointerAction::move_to(p.clone()).into(),
        PointerAction::button(Button::Code(300), true).into(),
        PointerAction::multi_click(Some(p.clone()), Button::Right, 2).into(),
        PointerAction::Axis {
            dx: 0.0,
            dy: 15.0,
            source: ScrollSource::finger(false),
            value120: Some((0, 120)),
        }
        .into(),
        KeyAction::Press {
            key: Key::name("a"),
        }
        .into(),
        KeyAction::Release { key: Key::code(30) }.into(),
        KeyAction::tap("Return").into(),
        KeyAction::combo(Combo::parse("ctrl+shift+t").unwrap_or_else(|e| panic!("{e}"))).into(),
        Action::Type {
            text: "héllo\n".into(),
            cadence: TypingCadence::wpm(90.0).with_jitter(0.3, 7),
            via: TextVia::Keys,
        },
        Action::Type {
            text: "x".into(),
            cadence: TypingCadence::default(),
            via: TextVia::TextInput,
        },
        TouchAction::Down {
            id: 1,
            pos: p.clone(),
        }
        .into(),
        TouchAction::Motion {
            id: 1,
            pos: p.clone(),
        }
        .into(),
        TouchAction::Up { id: 1 }.into(),
        TouchAction::tap(p.clone()).into(),
        TouchAction::glide(paths().remove(0), ms(200)).into(),
        ScrollGlide::new(VelocityCurve::ramp(0.0, 2000.0), secs(1.5))
            .at(p.clone())
            .axis(ScrollAxis::Horizontal)
            .source(ScrollSource::Continuous)
            .into(),
        Drag::line(p.clone(), Position::output(50.0, 50.0), ms(300))
            .button(Button::Middle)
            .easing(Easing::EaseOut)
            .into(),
        Fling::new(4000.0).into(),
        Gesture::pinch(p.clone(), 2.0, ms(400)).into(),
        Gesture::swipe(300.0, 0.0, ms(250)).into(),
        Gesture::hold(ms(100)).into(),
        OutputAction::SetMode {
            output: None,
            w: 2560,
            h: 1440,
            refresh: Hz(143.912),
        }
        .into(),
        OutputAction::SetScale {
            output: Some(OutputId(1)),
            scale: 1.5,
        }
        .into(),
        OutputAction::SetTransform {
            output: None,
            transform: Transform::Flipped270,
        }
        .into(),
        OutputAction::SetTiming {
            output: None,
            timing: TimingConfig::vrr(Hz(48.0), Hz(144.0)).with_callbacks(
                CallbackPolicy::BeforeVblank {
                    lead: Nanos::from_millis(4),
                },
            ),
        }
        .into(),
        OutputAction::Enable { output: None }.into(),
        OutputAction::Disable {
            output: Some(OutputId(0)),
        }
        .into(),
        WindowAction::Resize {
            window: win(),
            size: Size::new(800, 600),
        }
        .into(),
        WindowAction::ResizeStorm {
            window: win(),
            from: Size::new(400, 300),
            to: Size::new(1200, 900),
            duration: secs(2.0),
            rate_hz: 60.0,
            easing: Easing::EaseInOut,
        }
        .into(),
        WindowAction::Focus { window: win() }.into(),
        WindowAction::Maximize {
            window: WindowRef::Focused,
        }
        .into(),
        WindowAction::Fullscreen {
            window: WindowRef::Any,
        }
        .into(),
        WindowAction::Floating {
            window: win(),
            rect: Rect::new(10, 20, 300, 200),
        }
        .into(),
        WindowAction::Suspend {
            window: win(),
            suspended: true,
        }
        .into(),
        WindowAction::Close { window: win() }.into(),
        Action::mark("start"),
        Action::Capture {
            target: CaptureTarget::Window { window: win() },
            label: Some("shot".into()),
            max_dim: Some(1024),
        },
        Action::capture(CaptureTarget::Roi {
            roi: Roi::output(None),
        }),
        Action::sleep(ms(16)),
        Action::Repeat {
            count: 3,
            steps: vec![Step::new(Action::mark("tick")).anchor(Anchor::After { after: ms(5) })],
        },
        ClipboardAction::text("copied").into(),
    ];
    for g in paths() {
        out.push(PointerGlide::new(g, ms(500)).rate(250.0).into());
    }
    for w in waits() {
        out.push(w.into());
    }
    out
}

#[test]
fn every_action_round_trips() {
    for a in actions() {
        rt(&a);
    }
}

#[test]
fn every_target_round_trips() {
    for p in positions() {
        rt(&p);
    }
    for r in rois() {
        rt(&r);
    }
    for w in [
        WindowRef::id(WindowId(7)),
        win(),
        WindowRef::title_regex("x"),
        WindowRef::Focused,
        WindowRef::Any,
    ] {
        rt(&w);
    }
    for c in [
        CaptureTarget::Output { output: None },
        CaptureTarget::Window { window: win() },
        CaptureTarget::Roi {
            roi: Roi::output(None),
        },
    ] {
        rt(&c);
    }
}

#[test]
fn every_motion_curve_round_trips() {
    for p in paths() {
        rt(&p);
    }
    for e in [
        Easing::Linear,
        Easing::EaseIn,
        Easing::EaseOut,
        Easing::EaseInOut,
        Easing::Cubic {
            x1: 0.2,
            y1: 0.0,
            x2: 0.8,
            y2: 1.0,
        },
    ] {
        rt(&e);
    }
    for v in [
        VelocityCurve::constant(100.0),
        VelocityCurve::ramp(1.0, 2.0),
        VelocityCurve::keyframes([(0.0, 0.0), (1.0, 5.0)]),
    ] {
        rt(&v);
    }
    for s in [
        ScrollSource::wheel(true),
        ScrollSource::wheel(false),
        ScrollSource::finger(true),
        ScrollSource::Continuous,
    ] {
        rt(&s);
    }
    for b in [
        Button::Left,
        Button::Right,
        Button::Middle,
        Button::Side,
        Button::Extra,
        Button::Forward,
        Button::Back,
        Button::Code(999),
    ] {
        rt(&b);
    }
    for t in [
        Transform::Normal,
        Transform::Rotate90,
        Transform::Rotate180,
        Transform::Rotate270,
        Transform::Flipped,
        Transform::Flipped90,
        Transform::Flipped180,
        Transform::Flipped270,
    ] {
        rt(&t);
    }
}

#[test]
fn waits_conditions_triggers_and_probes_round_trip() {
    for c in conditions() {
        rt(&c);
    }
    for w in waits() {
        rt(&w);
    }
    let events = vec![
        TriggerEvent::Presented { window: None },
        TriggerEvent::Committed {
            window: Some(win()),
        },
        TriggerEvent::RoiChanged {
            roi: Roi::window(win()),
            threshold: 0.05,
        },
        TriggerEvent::InputSent,
        TriggerEvent::WindowMapped { matcher: win() },
        TriggerEvent::AppMark {
            name: "frame".into(),
        },
        TriggerEvent::Vblank {
            every: 2,
            output: Some(OutputId(1)),
        },
    ];
    for e in events {
        rt(&Trigger::on(e, [Action::mark("hit")]).limit(10).delay(ms(2)));
    }
    let detectors = vec![
        Detector::AnyDamage {
            roi: Roi::window(win()),
        },
        Detector::PixelsChanged {
            roi: Roi::output(None),
            threshold: 0.01,
        },
        Detector::PixelsMatch {
            roi: Roi::output(None),
            condition: PixelCondition::changed(),
        },
        Detector::AppMark { name: None },
    ];
    let phases = [
        PhaseSpec::Immediate,
        PhaseSpec::Fixed { offset: ms(3) },
        PhaseSpec::Random { seed: 9 },
        PhaseSpec::Sweep { steps: 16 },
    ];
    for (d, phase) in detectors.into_iter().zip(phases) {
        let probe = ProbeSpec::new(PointerAction::move_to(Position::output(1.0, 1.0)).into(), d)
            .phase(phase)
            .repeat(20)
            .alternate(PointerAction::move_to(Position::output(2.0, 2.0)).into())
            .label("hover");
        rt(&probe);
        rt(&Action::Probe(probe));
    }
}

#[test]
fn anchors_and_scenarios_round_trip() {
    let anchors = vec![
        Anchor::Immediately,
        Anchor::At { at: secs(2.0) },
        Anchor::After { after: us(1500) },
        Anchor::NextVblank {
            output: Some(OutputId(1)),
            phase: ms(3),
        },
        Anchor::OnEvent(Wait::window(win())),
    ];
    let steps: Vec<Step> = anchors
        .into_iter()
        .zip(actions())
        .map(|(anchor, action)| Step::new(action).anchor(anchor).label("s"))
        .collect();
    let scenario = Scenario {
        name: "all".into(),
        steps,
        triggers: vec![Trigger::on(TriggerEvent::InputSent, [Action::mark("x")])],
        timeout: Some(secs(30.0)),
    };
    rt(&scenario);
    let pretty = scenario.to_json_pretty();
    let back = Scenario::from_json(&pretty).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(back, scenario);
}

#[test]
fn durations_serialize_as_unit_strings() {
    let json = serde_json::to_value(Action::sleep(ms(250))).unwrap_or_default();
    assert_eq!(json["duration"], "250ms");
    let json = serde_json::to_value(Anchor::At { at: us(1500) }).unwrap_or_default();
    assert_eq!(json["at"], "1.5ms");
    let json = serde_json::to_value(Anchor::At { at: secs(2.0) }).unwrap_or_default();
    assert_eq!(json["at"], "2s");
}

#[test]
fn schema_covers_the_language() {
    let schema = serde_json::to_string(&Scenario::json_schema()).unwrap_or_default();
    for word in [
        "scroll_glide",
        "next_vblank",
        "pixels_match",
        "ease_in_out",
        "Dur",
        "Combo",
    ] {
        assert!(schema.contains(word), "{word}");
    }
}
