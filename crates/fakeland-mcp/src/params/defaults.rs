//! Default values of tool parameters.

pub fn wait_ms() -> u64 {
    10_000
}

pub fn lines() -> usize {
    100
}

pub fn max_dim() -> u32 {
    1280
}

pub fn settle_ms() -> u64 {
    100
}

pub fn quiet_ms() -> u64 {
    250
}

pub fn one() -> u32 {
    1
}

pub fn drag_ms() -> u64 {
    400
}

pub fn wpm() -> f64 {
    600.0
}

pub fn yes() -> bool {
    true
}

pub fn scenario_ms() -> u64 {
    60_000
}

pub fn repeat() -> u32 {
    10
}

pub fn stats_ms() -> u64 {
    5_000
}

pub fn focused() -> String {
    "focused".to_owned()
}
