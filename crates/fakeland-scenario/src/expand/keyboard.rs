//! Expansion of keyboard actions: presses, releases, taps and combinations.

use fakeland_types::{Dur, Nanos};

use super::{ExpandError, InputProgram, Primitive};
use crate::{Combo, Key};

pub(super) fn key(p: &mut InputProgram, at: Nanos, key: Key, pressed: bool) {
    p.push(at, Primitive::Key { key, pressed });
}

pub(super) fn single(k: &Key, pressed: bool) -> InputProgram {
    let mut p = InputProgram::new();
    key(&mut p, Nanos::ZERO, k.clone(), pressed);
    p
}

pub(super) fn tap(k: &Key, hold: Dur) -> Result<InputProgram, ExpandError> {
    if hold.0.is_negative() {
        return Err(ExpandError::NegativeDuration("key hold"));
    }
    let mut p = single(k, true);
    key(&mut p, hold.0, k.clone(), false);
    Ok(p)
}

pub(super) fn combo(combo: &Combo, hold: Dur, gap: Dur) -> Result<InputProgram, ExpandError> {
    if hold.0.is_negative() || gap.0.is_negative() {
        return Err(ExpandError::NegativeDuration("combo hold and gap"));
    }
    let mut p = InputProgram::new();
    let mut at = Nanos::ZERO;
    for m in &combo.modifiers {
        key(&mut p, at, m.key(), true);
        at += gap.0;
    }
    key(&mut p, at, combo.key.clone(), true);
    at += hold.0;
    key(&mut p, at, combo.key.clone(), false);
    for m in combo.modifiers.iter().rev() {
        at += gap.0;
        key(&mut p, at, m.key(), false);
    }
    Ok(p)
}
