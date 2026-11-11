//! Popup open on a synthetic menu.

use fakeland_trace::InputKind;

use super::*;
use crate::analyzer::Analyzer;
use crate::testing::{CommitOpts, SURF, Synth, at_ms};

#[test]
fn press_to_popup_present() {
    let mut s = Synth::new();
    s.input(
        at_ms(10.0),
        InputKind::PointerButton,
        Some((5.0, 5.0)),
        "button 273 pressed",
    );
    s.input(
        at_ms(50.0),
        InputKind::PointerButton,
        Some((5.0, 5.0)),
        "button 273 pressed",
    );
    let popup = s.surface(at_ms(62.0), "xdg_popup", Some(SURF));
    let c = s.commit(
        at_ms(70.0),
        CommitOpts {
            surface: popup,
            ..CommitOpts::default()
        },
    );
    s.present(c, 5);
    let r = PopupOpen::default().analyze(&s.analysis());
    assert_eq!(r.opened, 1);
    assert_eq!(r.unanswered_presses, 1);
    let p = &r.popups[0];
    assert_eq!(p.role, "xdg_popup");
    assert!((p.map_ms.unwrap() - 12.0).abs() < 1e-6);
    assert!((p.first_buffer_ms.unwrap() - 20.0).abs() < 1e-6);
    let present = 5.0 * 1000.0 / 60.0 - 50.0;
    assert!((p.first_present_ms.unwrap() - present).abs() < 1e-3);
    assert!(r.first_present_ms.is_some());
}
