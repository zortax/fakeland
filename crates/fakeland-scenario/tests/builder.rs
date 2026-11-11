//! The fluent Rust builder.

use fakeland_scenario::prelude::*;

#[test]
fn builder_reads_like_the_bench() {
    let win = WindowRef::app_id("org.my.app");
    let s = Scenario::builder("scroll")
        .wait_for_window(win.clone())
        .settle(ms(250))
        .at_next_vblank()
        .label("move")
        .pointer_move(Position::center(win.clone()))
        .scroll_glide(
            ScrollGlide::new(
                VelocityCurve::keyframes([(0.0, 0.0), (0.3, 2400.0), (1.0, 2400.0)]),
                secs(2.0),
            )
            .source(ScrollSource::wheel(true)),
        )
        .after(ms(100))
        .wait(Wait::pixels_stable(Roi::window(win.clone()), 3))
        .screenshot()
        .timeout(secs(20.0))
        .build()
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.steps.len(), 6);
    assert_eq!(s.steps[0].anchor, Anchor::Immediately);
    assert_eq!(
        s.steps[2].anchor,
        Anchor::NextVblank {
            output: None,
            phase: Dur::ZERO
        }
    );
    assert_eq!(s.steps[2].label.as_deref(), Some("move"));
    assert_eq!(s.steps[3].anchor, Anchor::Immediately);
    assert_eq!(s.steps[3].label, None);
    assert_eq!(s.steps[4].anchor, Anchor::After { after: ms(100) });
    assert_eq!(s.timeout, Some(secs(20.0)));
}

#[test]
fn repeat_and_triggers() {
    let s = Scenario::builder("ping-pong")
        .repeat(10, |b| {
            b.at_vblank_phase(ms(4))
                .pointer_move(Position::output(100.0, 100.0))
                .wait(Wait::presented(None))
                .pointer_move(Position::output(300.0, 100.0))
                .combo("ctrl+z")
        })
        .trigger(
            Trigger::on(
                TriggerEvent::RoiChanged {
                    roi: Roi::output(None),
                    threshold: 0.01,
                },
                [Action::mark("changed")],
            )
            .limit(20),
        )
        .build()
        .unwrap_or_else(|e| panic!("{e}"));
    let Action::Repeat { count, steps } = &s.steps[0].action else {
        panic!("repeat expected");
    };
    assert_eq!(*count, 10);
    assert_eq!(steps.len(), 4);
    assert_eq!(
        steps[0].anchor,
        Anchor::NextVblank {
            output: None,
            phase: ms(4)
        }
    );
    assert_eq!(s.triggers.len(), 1);
}

#[test]
fn convenience_steps() {
    let p = Position::output(10.0, 20.0);
    let s = Scenario::builder("all")
        .click(p.clone())
        .double_click(p.clone())
        .right_click(p.clone())
        .click_here(Button::Middle)
        .button(Button::Left, true)
        .button(Button::Left, false)
        .axis(0.0, 15.0, ScrollSource::wheel(false))
        .key_press("shift")
        .key_tap('x')
        .key_release("shift")
        .type_text("hi")
        .type_with("slow", TypingCadence::wpm(30.0))
        .commit_text("ünïcödé")
        .tap(p.clone())
        .touch(TouchAction::Up { id: 0 })
        .glide_to(p.clone(), Position::output(50.0, 50.0), ms(100))
        .drag(Drag::line(p.clone(), Position::output(60.0, 60.0), ms(100)))
        .fling(Fling::new(3000.0))
        .gesture(Gesture::swipe(100.0, 0.0, ms(100)))
        .resize(WindowRef::Focused, Size::new(640, 480))
        .window(WindowAction::Maximize {
            window: WindowRef::Focused,
        })
        .output(OutputAction::SetScale {
            output: None,
            scale: 2.0,
        })
        .mark("m")
        .capture(CaptureTarget::Window {
            window: WindowRef::Focused,
        })
        .probe(ProbeSpec::new(
            PointerAction::move_to(p).into(),
            Detector::AppMark { name: None },
        ))
        .wait_frames(2)
        .sleep(ms(5))
        .clipboard_text("clip")
        .at(secs(5.0))
        .mark("late")
        .on(Wait::app_mark("ready"))
        .mark("ready")
        .build()
        .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(s.steps.len(), 30);
    assert_eq!(s.steps[28].anchor, Anchor::At { at: secs(5.0) });
    assert_eq!(s.steps[29].anchor, Anchor::OnEvent(Wait::app_mark("ready")));
    let Action::Key(KeyAction::Press { key }) = &s.steps[7].action else {
        panic!("press expected");
    };
    assert_eq!(key, &Key::Name("Shift_L".into()));
}

#[test]
fn build_reports_errors() {
    let err = Scenario::builder("bad").mark("a").combo("hyper+q").build();
    assert!(
        matches!(err, Err(ScenarioError::Combo { ref location, .. }) if location == "steps[1]")
    );
    let err = Scenario::builder("bad")
        .glide(PointerGlide::new(Path::polyline([]), ms(10)))
        .build();
    assert!(matches!(err, Err(ScenarioError::Expand { .. })));
    let err = Scenario::builder("bad")
        .probe(ProbeSpec::new(
            Action::mark("x"),
            Detector::AppMark { name: None },
        ))
        .build();
    assert!(matches!(err, Err(ScenarioError::Invalid { .. })));
    let err = Scenario::builder("bad").repeat(0, |b| b.mark("x")).build();
    assert!(matches!(err, Err(ScenarioError::Invalid { .. })));
    let s = Scenario::builder("unchecked")
        .repeat(0, |b| b)
        .build_unchecked();
    assert_eq!(s.steps.len(), 1);
}
