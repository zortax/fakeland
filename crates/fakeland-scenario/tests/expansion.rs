//! Expansion of input actions into primitive programs: endpoints, counts, timing and totals.

use fakeland_scenario::prelude::*;
use proptest::prelude::*;

fn program(action: impl Into<Action>) -> InputProgram {
    expand(&action.into())
        .unwrap_or_else(|e| panic!("{e}"))
        .unwrap_or_else(|| panic!("input program expected"))
}

fn motions(p: &InputProgram) -> Vec<(Nanos, (f64, f64))> {
    p.events
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::PointerMotion { pos } => Some((e.offset, pos.coords())),
            _ => None,
        })
        .collect()
}

/// An axis event: offset, dx, dy, value120 and the stop flag.
type AxisEvent = (Nanos, f64, f64, Option<(i32, i32)>, bool);

fn axes(p: &InputProgram) -> Vec<AxisEvent> {
    p.events
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::PointerAxis {
                dx,
                dy,
                value120,
                stop,
                ..
            } => Some((e.offset, *dx, *dy, *value120, *stop)),
            _ => None,
        })
        .collect()
}

fn keys(p: &InputProgram) -> Vec<(Nanos, String, bool)> {
    p.events
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::Key {
                key: Key::Name(n),
                pressed,
            } => Some((e.offset, n.clone(), *pressed)),
            _ => None,
        })
        .collect()
}

fn line() -> Path {
    Path::line(Position::output(0.0, 0.0), Position::output(100.0, 50.0))
}

fn sorted(p: &InputProgram) -> bool {
    p.events.windows(2).all(|w| w[0].offset <= w[1].offset)
}

#[test]
fn glide_hits_both_ends_at_the_rate() {
    let p = program(PointerGlide::new(line(), secs(1.0)));
    let m = motions(&p);
    assert_eq!(m.len(), 126);
    assert_eq!(m[0], (Nanos::ZERO, (0.0, 0.0)));
    assert_eq!(m[125], (Nanos::from_secs(1), (100.0, 50.0)));
    assert!(
        m.windows(2)
            .all(|w| w[1].0 - w[0].0 == Nanos::from_millis(8))
    );
    let frames = p
        .primitives()
        .filter(|e| matches!(e, Primitive::PointerFrame))
        .count();
    assert_eq!(frames, 126);
    assert!(matches!(p.events[1].primitive, Primitive::PointerFrame));
    assert!(sorted(&p));
}

#[test]
fn eased_glide_moves_slowly_at_the_ends() {
    let p = program(
        PointerGlide::new(line(), ms(100))
            .easing(Easing::EaseInOut)
            .rate(100.0),
    );
    let m = motions(&p);
    assert_eq!(m.len(), 11);
    assert!((m[5].1.0 - 50.0).abs() < 1e-6);
    assert!(m[1].1.0 < 10.0);
    assert!(m[9].1.0 > 90.0);
    assert_eq!(m[10].1, (100.0, 50.0));
}

#[test]
fn glide_positions_stay_in_the_window_frame() {
    let w = WindowRef::app_id("foot");
    let path = Path::line(
        Position::fraction(w.clone(), 0.1, 0.5),
        Position::fraction(w.clone(), 0.9, 0.5),
    );
    let p = program(PointerGlide::new(path, ms(50)).rate(40.0));
    let Primitive::PointerMotion { pos } = &p.events[2].primitive else {
        panic!("motion expected");
    };
    assert_eq!(pos, &Position::fraction(w, 0.5, 0.5));
}

#[test]
fn mixed_frames_do_not_expand() {
    let path = Path::line(Position::output(0.0, 0.0), Position::center(WindowRef::Any));
    let err = expand(&PointerGlide::new(path, ms(50)).into());
    assert_eq!(err, Err(ExpandError::Motion(MotionError::MixedFrames)));
}

#[test]
fn double_click_timing() {
    let at = Position::output(10.0, 10.0);
    let p = program(PointerAction::multi_click(Some(at), Button::Left, 2));
    let events: Vec<(Nanos, &Primitive)> = p
        .events
        .iter()
        .filter(|e| !matches!(e.primitive, Primitive::PointerFrame))
        .map(|e| (e.offset, &e.primitive))
        .collect();
    let press = |pressed| Primitive::PointerButton {
        button: Button::Left,
        pressed,
    };
    assert!(matches!(events[0].1, Primitive::PointerMotion { .. }));
    assert_eq!(events[1], (Nanos::ZERO, &press(true)));
    assert_eq!(events[2], (Nanos::from_millis(40), &press(false)));
    assert_eq!(events[3], (Nanos::from_millis(120), &press(true)));
    assert_eq!(events[4], (Nanos::from_millis(160), &press(false)));
    let p = program(PointerAction::Click {
        at: None,
        button: Button::Left,
        count: 2,
        interval: ms(100),
        hold: ms(80),
    });
    assert_eq!(p.events[2].offset, Nanos::from_millis(50));
    assert!(expand(&PointerAction::multi_click(None, Button::Left, 0).into()).is_err());
}

#[test]
fn single_wheel_axis_derives_value120() {
    let p = program(PointerAction::axis(0.0, 30.0, ScrollSource::wheel(true)));
    assert_eq!(
        axes(&p),
        vec![(Nanos::ZERO, 0.0, 30.0, Some((0, 240)), false)]
    );
    let p = program(PointerAction::axis(5.0, 0.0, ScrollSource::Continuous));
    assert_eq!(axes(&p), vec![(Nanos::ZERO, 5.0, 0.0, None, false)]);
}

#[test]
fn hires_wheel_glide_matches_the_integral() {
    let p = program(ScrollGlide::constant(2400.0, secs(2.0)));
    let a = axes(&p);
    assert_eq!(a.len(), 240);
    assert!(
        a.iter()
            .all(|e| e.3 == Some((0, 160)) && (e.2 - 20.0).abs() < 1e-9)
    );
    let total: i32 = a.iter().map(|e| e.3.map_or(0, |v| v.1)).sum();
    assert_eq!(total, 38_400);
    let px: f64 = a.iter().map(|e| e.2).sum();
    assert!((px - 4800.0).abs() < 1e-6);
    assert_eq!(a.last().map(|e| e.0), Some(Nanos::from_secs(2)));
}

#[test]
fn lowres_wheel_sends_whole_detents() {
    let glide = ScrollGlide::constant(1000.0, secs(1.01)).source(ScrollSource::wheel(false));
    let a = axes(&program(glide));
    assert!(
        a.iter()
            .all(|e| e.3.is_some_and(|v| v.1 % 120 == 0 && v.1 != 0))
    );
    let detents: i32 = a.iter().map(|e| e.3.map_or(0, |v| v.1 / 120)).sum();
    assert_eq!(detents, 67);
    let px: f64 = a.iter().map(|e| e.2).sum();
    assert!((px - 1005.0).abs() < 1e-9);
    let hires = axes(&program(ScrollGlide::constant(1000.0, secs(1.01))));
    let v120: i32 = hires.iter().map(|e| e.3.map_or(0, |v| v.1)).sum();
    assert_eq!(v120, 8080);
}

#[test]
fn finger_glide_ends_with_axis_stop() {
    let glide = ScrollGlide::new(VelocityCurve::ramp(3000.0, 0.0), secs(1.0))
        .axis(ScrollAxis::Horizontal)
        .source(ScrollSource::finger(true));
    let a = axes(&program(glide));
    let (stop, motion) = a.split_last().unwrap_or_else(|| panic!("events expected"));
    assert!(stop.4);
    assert_eq!(
        stop.0,
        Nanos::from_secs(1) + Nanos::from_secs_f64(1.0 / 120.0)
    );
    assert!(motion.iter().all(|e| !e.4 && e.3.is_none() && e.2 == 0.0));
    let px: f64 = motion.iter().map(|e| e.1).sum();
    assert!((px - 1500.0).abs() < 1e-6);
    let glide = ScrollGlide::constant(500.0, ms(500)).source(ScrollSource::finger(false));
    let a = axes(&program(glide));
    let last = a.last().map(|e| e.0).unwrap_or_default();
    assert_eq!(last, Nanos::from_millis(650));
    let a = axes(&program(
        ScrollGlide::constant(500.0, ms(500)).source(ScrollSource::Continuous),
    ));
    assert!(a.iter().all(|e| !e.4));
}

#[test]
fn fling_decays_and_stops() {
    let p = program(Fling::new(5000.0));
    let a = axes(&p);
    let (stop, motion) = a.split_last().unwrap_or_else(|| panic!("events expected"));
    assert!(stop.4);
    assert!(motion.windows(2).all(|w| w[1].2 < w[0].2));
    let px: f64 = motion.iter().map(|e| e.2).sum();
    let expected = 5000.0 * 0.325 * (1.0 - (-1.0f64 / 0.325).exp());
    assert!((px - expected).abs() < 1e-6, "{px} vs {expected}");
    assert!(matches!(
        p.events.last().map(|e| &e.primitive),
        Some(Primitive::PointerFrame)
    ));
}

#[test]
fn typing_follows_the_wpm() {
    let text = "hello";
    let p = program(Action::Type {
        text: text.into(),
        cadence: TypingCadence::wpm(60.0),
        via: TextVia::Keys,
    });
    let k = keys(&p);
    let presses: Vec<Nanos> = k.iter().filter(|e| e.2).map(|e| e.0).collect();
    assert_eq!(
        presses,
        [0, 200, 400, 600, 800].map(Nanos::from_millis).to_vec()
    );
    let releases: Vec<Nanos> = k.iter().filter(|e| !e.2).map(|e| e.0).collect();
    assert_eq!(
        releases,
        [20, 220, 420, 620, 820].map(Nanos::from_millis).to_vec()
    );
    let names: Vec<&str> = k.iter().filter(|e| e.2).map(|e| e.1.as_str()).collect();
    assert_eq!(names, vec!["h", "e", "l", "l", "o"]);
    assert!(sorted(&p));
}

#[test]
fn typing_jitter_is_seeded() {
    let cadence = |seed| TypingCadence::wpm(60.0).with_jitter(0.5, seed);
    let presses = |seed| -> Vec<Nanos> {
        keys(&program(Action::Type {
            text: "jitter test".into(),
            cadence: cadence(seed),
            via: TextVia::Keys,
        }))
        .into_iter()
        .filter(|e| e.2)
        .map(|e| e.0)
        .collect()
    };
    let a = presses(1);
    assert_eq!(a, presses(1));
    assert_ne!(a, presses(2));
    assert_eq!(a[0], Nanos::ZERO);
    for (i, t) in a.iter().enumerate().skip(1) {
        let nominal = Nanos::from_millis(200 * i as i64);
        assert!((*t - nominal).abs() <= Nanos::from_millis(50));
    }
}

#[test]
fn fast_typing_keeps_hold_below_the_gap() {
    let p = program(Action::Type {
        text: "aa".into(),
        cadence: TypingCadence::wpm(1200.0).with_hold(ms(20)),
        via: TextVia::Keys,
    });
    let k = keys(&p);
    assert_eq!(
        k.iter().map(|e| (e.0, e.2)).collect::<Vec<_>>(),
        vec![
            (Nanos::ZERO, true),
            (Nanos::from_millis(5), false),
            (Nanos::from_millis(10), true),
            (Nanos::from_millis(30), false)
        ]
    );
    let shifted = keys(&program(Action::type_text("A €")));
    let names: Vec<&str> = shifted
        .iter()
        .filter(|e| e.2)
        .map(|e| e.1.as_str())
        .collect();
    assert_eq!(names, vec!["A", "space", "U20AC"]);
    let p = program(Action::Type {
        text: "fast".into(),
        cadence: TypingCadence::default(),
        via: TextVia::TextInput,
    });
    assert_eq!(p.events.len(), 1);
    assert_eq!(
        p.events[0].primitive,
        Primitive::Text {
            text: "fast".into()
        }
    );
}

#[test]
fn combo_presses_modifiers_first() {
    let combo = Combo::parse("ctrl+shift+t").unwrap_or_else(|e| panic!("{e}"));
    let k = keys(&program(KeyAction::combo(combo)));
    let expected = [
        (0, "Control_L", true),
        (10, "Shift_L", true),
        (20, "t", true),
        (50, "t", false),
        (60, "Shift_L", false),
        (70, "Control_L", false),
    ]
    .map(|(t, n, p)| (Nanos::from_millis(t), n.to_owned(), p));
    assert_eq!(k, expected.to_vec());
}

#[test]
fn drag_presses_glides_and_releases() {
    let from = Position::output(10.0, 10.0);
    let to = Position::output(110.0, 10.0);
    let p = program(Drag::line(from, to, ms(200)));
    let buttons: Vec<(Nanos, bool)> = p
        .events
        .iter()
        .filter_map(|e| match e.primitive {
            Primitive::PointerButton { pressed, .. } => Some((e.offset, pressed)),
            _ => None,
        })
        .collect();
    assert_eq!(
        buttons,
        vec![(Nanos::ZERO, true), (Nanos::from_millis(300), false)]
    );
    let m = motions(&p);
    assert_eq!(m[0], (Nanos::ZERO, (10.0, 10.0)));
    assert_eq!(m[1], (Nanos::from_millis(50), (10.0, 10.0)));
    assert_eq!(
        m.last().copied(),
        Some((Nanos::from_millis(250), (110.0, 10.0)))
    );
    assert_eq!(m.len(), 2 + 25);
    assert!(sorted(&p));
}

#[test]
fn pinch_and_swipe_reach_their_totals() {
    let p = program(Gesture::Pinch {
        center: Position::output(50.0, 50.0),
        scale_from: 1.0,
        scale_to: 2.5,
        rotation: 90.0,
        duration: ms(500),
        fingers: 2,
        easing: Easing::EaseInOut,
        rate_hz: 100.0,
    });
    let updates: Vec<(f64, f64)> = p
        .primitives()
        .filter_map(|e| match e {
            Primitive::PinchUpdate {
                scale, rotation, ..
            } => Some((*scale, *rotation)),
            _ => None,
        })
        .collect();
    assert_eq!(updates.len(), 50);
    assert!((updates[49].0 - 2.5).abs() < 1e-12);
    assert!((updates.iter().map(|u| u.1).sum::<f64>() - 90.0).abs() < 1e-9);
    assert!(matches!(
        p.events[2].primitive,
        Primitive::PinchBegin { fingers: 2 }
    ));
    assert_eq!(
        p.events.last().map(|e| (e.offset, e.primitive.clone())),
        Some((
            Nanos::from_millis(500),
            Primitive::PinchEnd { cancelled: false }
        ))
    );
    let p = program(Gesture::swipe(-300.0, 120.0, ms(250)));
    let (dx, dy) = p.primitives().fold((0.0, 0.0), |acc, e| match e {
        Primitive::SwipeUpdate { dx, dy } => (acc.0 + dx, acc.1 + dy),
        _ => acc,
    });
    assert!((dx + 300.0).abs() < 1e-9 && (dy - 120.0).abs() < 1e-9);
    let p = program(Gesture::hold(ms(80)));
    assert_eq!(p.duration(), Nanos::from_millis(80));
    let bad = Gesture::Pinch {
        center: Position::output(0.0, 0.0),
        scale_from: 1.0,
        scale_to: 2.0,
        rotation: 0.0,
        duration: ms(10),
        fingers: 1,
        easing: Easing::Linear,
        rate_hz: 60.0,
    };
    assert!(matches!(
        expand(&bad.into()),
        Err(ExpandError::Fingers { .. })
    ));
}

#[test]
fn touch_glide_downs_moves_and_lifts() {
    let p = program(TouchAction::glide(line(), ms(100)));
    let kinds: Vec<&Primitive> = p
        .primitives()
        .filter(|e| !matches!(e, Primitive::TouchFrame))
        .collect();
    assert!(matches!(kinds[0], Primitive::TouchDown { id: 0, .. }));
    assert!(matches!(
        kinds[kinds.len() - 1],
        Primitive::TouchUp { id: 0 }
    ));
    assert_eq!(kinds.len(), 1 + 12 + 1);
    let p = program(TouchAction::tap(Position::output(1.0, 2.0)));
    assert_eq!(p.duration(), Nanos::from_millis(50));
}

#[test]
fn resize_storm_steps_through_sizes() {
    let p = program(WindowAction::ResizeStorm {
        window: WindowRef::Focused,
        from: Size::new(400, 300),
        to: Size::new(1200, 900),
        duration: secs(1.0),
        rate_hz: 60.0,
        easing: Easing::Linear,
    });
    let sizes: Vec<(Nanos, Size<Logical>)> = p
        .events
        .iter()
        .filter_map(|e| match &e.primitive {
            Primitive::WindowResize { size, .. } => Some((e.offset, *size)),
            _ => None,
        })
        .collect();
    assert_eq!(sizes.len(), 61);
    assert_eq!(sizes[0], (Nanos::ZERO, Size::new(400, 300)));
    assert_eq!(sizes[60], (Nanos::from_secs(1), Size::new(1200, 900)));
    assert!(sizes.windows(2).all(|w| w[0].1 != w[1].1));
    assert!(
        expand(
            &WindowAction::Resize {
                window: WindowRef::Focused,
                size: Size::new(0, 10)
            }
            .into()
        )
        .is_err()
    );
}

#[test]
fn non_input_actions_do_not_expand() {
    for a in [
        Action::mark("x"),
        Action::sleep(ms(1)),
        Wait::frames(1, None).into(),
        OutputAction::Enable { output: None }.into(),
        WindowAction::Focus {
            window: WindowRef::Any,
        }
        .into(),
    ] {
        assert_eq!(expand(&a), Ok(None));
        assert!(!a.is_input());
    }
}

fn curve() -> impl Strategy<Value = VelocityCurve> {
    prop_oneof![
        (-5000.0f64..5000.0).prop_map(VelocityCurve::constant),
        (-5000.0f64..5000.0, -5000.0f64..5000.0).prop_map(|(a, b)| VelocityCurve::ramp(a, b)),
        prop::collection::vec((0.0f64..1.0, -4000.0f64..4000.0), 1..6).prop_map(|mut pts| {
            pts.sort_by(|a, b| a.0.total_cmp(&b.0));
            VelocityCurve::keyframes(pts)
        }),
    ]
}

fn source() -> impl Strategy<Value = ScrollSource> {
    prop_oneof![
        any::<bool>().prop_map(ScrollSource::wheel),
        any::<bool>().prop_map(ScrollSource::finger),
        Just(ScrollSource::Continuous),
    ]
}

proptest! {
    #[test]
    fn scroll_total_matches_the_curve(
        velocity in curve(),
        duration_ms in 50i64..3000,
        rate in 30.0f64..1000.0,
        src in source(),
    ) {
        let duration = ms(duration_ms);
        let expected = velocity.total(duration.0.as_secs_f64());
        let glide = ScrollGlide::new(velocity, duration).source(src).rate(rate);
        let a = axes(&program(glide));
        let px: f64 = a.iter().map(|e| e.2).sum();
        let tolerance = match src {
            ScrollSource::Wheel { hires: true, detent } => detent / 240.0 + 1e-6,
            ScrollSource::Wheel { hires: false, detent } => detent / 2.0 + 1e-6,
            _ => 1e-6 * expected.abs().max(1.0),
        };
        prop_assert!((px - expected).abs() <= tolerance, "{} vs {}", px, expected);
        prop_assert!(a.windows(2).all(|w| w[0].0 <= w[1].0));
        let stops = a.iter().filter(|e| e.4).count();
        let finger = matches!(src, ScrollSource::Finger { .. });
        prop_assert_eq!(stops, usize::from(finger));
        if finger {
            prop_assert!(a.last().is_some_and(|e| e.4));
        }
    }

    #[test]
    fn glide_endpoints_are_exact(
        x0 in -2000.0f64..2000.0, y0 in -2000.0f64..2000.0,
        x1 in -2000.0f64..2000.0, y1 in -2000.0f64..2000.0,
        duration_ms in 0i64..2000, rate in 1.0f64..2000.0,
    ) {
        let path = Path::line(Position::output(x0, y0), Position::output(x1, y1));
        let p = program(PointerGlide::new(path, ms(duration_ms)).rate(rate).easing(Easing::EaseOut));
        let m = motions(&p);
        prop_assert_eq!(m[0].1, (x0, y0));
        let last = m[m.len() - 1].1;
        prop_assert!((last.0 - x1).abs() < 1e-9 && (last.1 - y1).abs() < 1e-9);
        prop_assert_eq!(m[m.len() - 1].0, Nanos::from_millis(duration_ms));
    }

    #[test]
    fn typing_presses_strictly_increase(
        text in "[a-zA-Z0-9 .,!?]{2,40}",
        wpm in 10.0f64..2000.0,
        jitter in 0.0f64..0.99,
        seed in any::<u64>(),
    ) {
        let p = program(Action::Type {
            text,
            cadence: TypingCadence::wpm(wpm).with_jitter(jitter, seed),
            via: TextVia::Keys,
        });
        let k = keys(&p);
        let presses: Vec<Nanos> = k.iter().filter(|e| e.2).map(|e| e.0).collect();
        prop_assert!(presses.windows(2).all(|w| w[0] < w[1]));
        prop_assert!(sorted(&p));
    }
}
